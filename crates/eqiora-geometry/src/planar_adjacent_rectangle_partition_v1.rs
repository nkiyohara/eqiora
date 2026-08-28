//! Canonical exact topology of two adjacent axis-aligned rectangles.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::canonical::{CANONICAL_ENCODING, WireEntitySet, WireLengthUnit, digest_with_schema};
use crate::region::canonical_entity_sets;
use crate::{
    CanonicalGeometryLimits, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet, PlanarFace,
    PlanarRegion,
};

const SCHEMA: &str = "eqiora.planar-adjacent-rectangle-partition-envelope/v1";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Exact two-region rectangle partition with one construction-owned interface.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanonicalPlanarAdjacentRectanglePartitionV1 {
    bounds: [[f64; 2]; 2],
    interface_x: f64,
    region: PlanarRegion,
    entity_sets: Vec<NamedEntitySet>,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CanonicalPlanarAdjacentRectanglePartitionV1 {
    pub(crate) fn new(
        mut bounds: [[f64; 2]; 2],
        mut interface_x: f64,
        entity_sets: Vec<NamedEntitySet>,
    ) -> Result<Self, Diagnostic> {
        for axis in &mut bounds {
            for value in axis.iter_mut() {
                if *value == 0.0 {
                    *value = 0.0;
                }
            }
            if !axis[0].is_finite() || !axis[1].is_finite() || axis[0] >= axis[1] {
                return Err(invalid("partition bounds must be finite strict intervals"));
            }
        }
        if interface_x == 0.0 {
            interface_x = 0.0;
        }
        if !interface_x.is_finite() || interface_x <= bounds[0][0] || interface_x >= bounds[0][1] {
            return Err(invalid(
                "partition interface must lie strictly inside the x interval",
            ));
        }
        let entity_sets = canonical_entity_sets(entity_sets, 6, 8, 2)?;
        validate_complete_membership(&entity_sets)?;
        let region = PlanarRegion::new(
            vec![
                [bounds[0][0], bounds[1][0]],
                [bounds[0][0], bounds[1][1]],
                [interface_x, bounds[1][0]],
                [interface_x, bounds[1][1]],
                [bounds[0][1], bounds[1][0]],
                [bounds[0][1], bounds[1][1]],
            ],
            vec![
                PlanarFace::new(vec![0, 2, 3, 1], Vec::new()),
                PlanarFace::new(vec![2, 4, 5, 3], Vec::new()),
            ],
            entity_sets.clone(),
            f64::MIN_POSITIVE,
        )?;
        let wire = WirePartitionV1 {
            schema: SCHEMA.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            length_unit: WireLengthUnit::Metre,
            bounds,
            interface_x,
            interface: WireInterfaceV1 {
                left_face: 0,
                left_edge: 1,
                right_face: 1,
                right_edge: 7,
                incidence: WireInterfaceIncidenceV1::OppositeParentOutward,
            },
            entity_sets: entity_sets.iter().map(WireEntitySet::from_set).collect(),
        };
        let bytes = serde_json::to_vec(&wire)
            .map_err(|error| invalid(format!("cannot serialize partition Geometry: {error}")))?;
        Ok(Self {
            bounds,
            interface_x,
            region,
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
            return Err(invalid("partition Geometry exceeds the decoder byte limit"));
        }
        let wire: WirePartitionV1 = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid partition Geometry JSON: {error}")))?;
        if wire.schema != SCHEMA
            || wire.encoding != CANONICAL_ENCODING
            || wire.length_unit != WireLengthUnit::Metre
            || wire.interface
                != (WireInterfaceV1 {
                    left_face: 0,
                    left_edge: 1,
                    right_face: 1,
                    right_edge: 7,
                    incidence: WireInterfaceIncidenceV1::OppositeParentOutward,
                })
            || wire.entity_sets.len() > limits.max_entity_sets
            || wire
                .entity_sets
                .iter()
                .map(|set| set.members.len())
                .sum::<usize>()
                > limits.max_entity_set_members
        {
            return Err(invalid(
                "unsupported or excessive partition Geometry contract",
            ));
        }
        let canonical = Self::new(
            wire.bounds,
            wire.interface_x,
            wire.entity_sets
                .into_iter()
                .map(|set| NamedEntitySet::new(set.name, set.dimension, set.members))
                .collect(),
        )?;
        if canonical.canonical_bytes() != bytes {
            return Err(invalid("partition Geometry JSON is not canonical"));
        }
        Ok(canonical)
    }

    pub(crate) const fn bounds(&self) -> &[[f64; 2]; 2] {
        &self.bounds
    }

    pub(crate) const fn interface_x(&self) -> f64 {
        self.interface_x
    }

    pub(crate) const fn region(&self) -> &PlanarRegion {
        &self.region
    }

    pub(crate) fn entity_sets(&self) -> &[NamedEntitySet] {
        &self.entity_sets
    }

    pub(crate) fn selections_form_opposite_parent_interface(
        &self,
        left_boundary: &NamedEntitySet,
        left_region: &NamedEntitySet,
        right_boundary: &NamedEntitySet,
        right_region: &NamedEntitySet,
    ) -> bool {
        let side = |boundary: &NamedEntitySet, region: &NamedEntitySet| match (
            boundary.members(),
            region.members(),
        ) {
            ([1], [0]) => Some(0_u8),
            ([7], [1]) => Some(1_u8),
            _ => None,
        };
        matches!(
            (
                side(left_boundary, left_region),
                side(right_boundary, right_region)
            ),
            (Some(0), Some(1)) | (Some(1), Some(0))
        )
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest
    }
}

fn validate_complete_membership(entity_sets: &[NamedEntitySet]) -> Result<(), Diagnostic> {
    let memberships = entity_sets
        .iter()
        .flat_map(|set| {
            set.members()
                .iter()
                .map(move |&member| (set.dimension(), member))
        })
        .collect::<Vec<_>>();
    let unique = memberships.iter().copied().collect::<BTreeSet<_>>();
    let expected = (0..8)
        .map(|edge| (EDGE_DIMENSION, edge))
        .chain((0..2).map(|face| (FACE_DIMENSION, face)))
        .collect::<BTreeSet<_>>();
    if unique != expected || memberships.len() != expected.len() {
        return Err(invalid(
            "partition naming must cover both regions and every parent-relative interface/exterior exactly once",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WirePartitionV1 {
    schema: String,
    encoding: String,
    length_unit: WireLengthUnit,
    bounds: [[f64; 2]; 2],
    interface_x: f64,
    interface: WireInterfaceV1,
    entity_sets: Vec<WireEntitySet>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireInterfaceV1 {
    left_face: u64,
    left_edge: u64,
    right_face: u64,
    right_edge: u64,
    incidence: WireInterfaceIncidenceV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireInterfaceIncidenceV1 {
    OppositeParentOutward,
}
