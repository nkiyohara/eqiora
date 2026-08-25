//! Canonical identity of one exact circular hole in an axis-aligned rectangle.
//!
//! The circle belongs to geometry meaning and is stored as centre and radius.
//! No polygon, mesh spacing, chord count, or approximation tolerance enters
//! this value or its identity. A later Realization may therefore replace
//! affine chords with curved elements without changing the Model's geometry.

use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::canonical::{CANONICAL_ENCODING, WireEntitySet, WireLengthUnit, digest_with_schema};
use crate::region::canonical_entity_sets;
use crate::{CanonicalGeometryLimits, NamedEntitySet};

const CIRCULAR_HOLE_SCHEMA: &str = "eqiora.planar-circular-hole-envelope/v1";
const CORNER_COUNT: usize = 4;
const OUTER_LOOP_INDEX_COUNT: usize = 4;
const BOUNDARY_COUNT: usize = 5;
const FACE_COUNT: usize = 1;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Exact axis-aligned rectangular planar region with one circular hole.
///
/// Geometry entities have one fixed enumeration:
///
/// - vertices: rectangle corners in lexicographic `(x, y)` order;
/// - boundaries: x-lower, x-upper, y-lower, y-upper, then the circle;
/// - faces: the one rectangle-minus-circle region.
///
/// Named entity sets use those indices. This value owns exact analytic
/// geometry only; a chordal or curved numerical representation is a separate
/// source-bound Realization.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CircularHoleGeometry {
    bounds: [[f64; 2]; 2],
    circle_center: [f64; 2],
    circle_radius_m: f64,
    entity_sets: Vec<NamedEntitySet>,
    tolerance_m: f64,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CircularHoleGeometry {
    /// Construct one exact rectangle-minus-circle geometry from semantic roles.
    ///
    /// Equal boundary names group their fixed roles into one entity set. This
    /// keeps role-to-entity wiring in the exact Geometry owner rather than in a
    /// language adapter or application demo.
    ///
    /// # Errors
    /// Returns the same diagnostics as [`Self::new`] after grouping the five
    /// boundary roles and the one full-dimensional region role.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_named_roles(
        bounds: [[f64; 2]; 2],
        circle_center: [f64; 2],
        circle_radius_m: f64,
        tolerance_m: f64,
        region: &str,
        x_lower: &str,
        x_upper: &str,
        y_lower: &str,
        y_upper: &str,
        hole: &str,
    ) -> Result<Self, Diagnostic> {
        let mut boundaries = BTreeMap::<String, Vec<usize>>::new();
        for (name, entity) in [
            (x_lower, 0),
            (x_upper, 1),
            (y_lower, 2),
            (y_upper, 3),
            (hole, 4),
        ] {
            boundaries.entry(name.to_owned()).or_default().push(entity);
        }
        let mut entity_sets = boundaries
            .into_iter()
            .map(|(name, members)| NamedEntitySet::new(name, crate::EDGE_DIMENSION, members))
            .collect::<Vec<_>>();
        entity_sets.push(NamedEntitySet::new(region, crate::FACE_DIMENSION, vec![0]));
        Self::new(
            bounds,
            circle_center,
            circle_radius_m,
            entity_sets,
            tolerance_m,
        )
    }

    /// Validate and canonicalize one exact circular-hole geometry.
    ///
    /// `bounds[axis]` is `[lower, upper]` in metres. The closed circle must
    /// lie strictly inside every rectangle side by more than `tolerance_m`.
    /// Signed zero in coordinates is normalized to positive zero.
    ///
    /// # Errors
    /// Returns `EQ0901` for non-finite or degenerate geometric data, an
    /// insufficient circle-to-side clearance, an invalid entity set, or an
    /// unexpected canonical serialization failure.
    pub(crate) fn new(
        mut bounds: [[f64; 2]; 2],
        mut circle_center: [f64; 2],
        circle_radius_m: f64,
        entity_sets: Vec<NamedEntitySet>,
        tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        normalize_zeros(&mut bounds, &mut circle_center);
        validate_geometry(&bounds, &circle_center, circle_radius_m, tolerance_m)?;
        let entity_sets =
            canonical_entity_sets(entity_sets, CORNER_COUNT, BOUNDARY_COUNT, FACE_COUNT)?;
        let wire = WireCircularHoleGeometryV1::from_parts(
            bounds,
            circle_center,
            circle_radius_m,
            &entity_sets,
            tolerance_m,
        );
        let bytes = serde_json::to_vec(&wire).map_err(|error| {
            invalid(format!(
                "cannot serialize canonical circular-hole geometry: {error}"
            ))
        })?;
        Ok(Self {
            bounds,
            circle_center,
            circle_radius_m,
            entity_sets,
            tolerance_m,
            digest: digest_with_schema(CIRCULAR_HOLE_SCHEMA, &bytes),
            bytes,
        })
    }

    /// Decode bounded, byte-for-byte canonical circular-hole geometry.
    ///
    /// The supplied bytes are parsed through closed wire vocabulary, checked
    /// against resource limits, reconstructed through [`Self::new`], and
    /// accepted only when reconstruction reproduces the input exactly.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed or unknown wire data, resource excess,
    /// invalid geometry or entity sets, or a noncanonical encoding.
    pub(crate) fn decode_canonical(
        bytes: &[u8],
        limits: CanonicalGeometryLimits,
    ) -> Result<Self, Diagnostic> {
        if bytes.len() > limits.max_bytes {
            return Err(invalid(format!(
                "circular-hole geometry has {} bytes, exceeding the {} byte decoder limit",
                bytes.len(),
                limits.max_bytes
            )));
        }
        let wire: WireCircularHoleGeometryV1 = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid circular-hole geometry JSON: {error}")))?;
        wire.check_contract()?;
        wire.check_limits(limits)?;
        let canonical = Self::new(
            wire.bounds,
            wire.circle.center,
            wire.circle.radius_m,
            wire.entity_sets
                .into_iter()
                .map(|set| NamedEntitySet::new(set.name, set.dimension, set.members))
                .collect(),
            wire.tolerance_m,
        )?;
        if canonical.bytes != bytes {
            return Err(invalid(
                "circular-hole geometry JSON is not the canonical encoding of its content",
            ));
        }
        Ok(canonical)
    }

    /// Exact `[lower, upper]` bounds for Cartesian axes x then y, in metres.
    #[must_use]
    pub(crate) const fn bounds(&self) -> &[[f64; 2]; 2] {
        &self.bounds
    }

    /// Exact circle centre `[x, y]` in metres.
    #[must_use]
    pub(crate) const fn circle_center(&self) -> [f64; 2] {
        self.circle_center
    }

    /// Exact circle radius in metres.
    #[must_use]
    pub(crate) const fn circle_radius_m(&self) -> f64 {
        self.circle_radius_m
    }

    /// Producer classification precision in metres.
    #[must_use]
    pub(crate) const fn tolerance_m(&self) -> f64 {
        self.tolerance_m
    }

    /// Canonically ordered named exact entity sets.
    #[must_use]
    pub(crate) fn entity_sets(&self) -> &[NamedEntitySet] {
        &self.entity_sets
    }

    /// Exact compact canonical JSON bytes.
    #[must_use]
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Complete domain-separated SHA-256 identity bytes.
    #[must_use]
    pub(crate) const fn digest_bytes(&self) -> [u8; 32] {
        self.digest
    }
}

fn normalize_zeros(bounds: &mut [[f64; 2]; 2], circle_center: &mut [f64; 2]) {
    for coordinate in bounds.iter_mut().flatten().chain(circle_center.iter_mut()) {
        if *coordinate == 0.0 {
            *coordinate = 0.0;
        }
    }
}

fn validate_geometry(
    bounds: &[[f64; 2]; 2],
    circle_center: &[f64; 2],
    circle_radius_m: f64,
    tolerance_m: f64,
) -> Result<(), Diagnostic> {
    if bounds.iter().flatten().any(|value| !value.is_finite())
        || circle_center.iter().any(|value| !value.is_finite())
    {
        return Err(invalid(
            "circular-hole coordinates and bounds must be finite metres",
        ));
    }
    if !circle_radius_m.is_finite() || circle_radius_m <= 0.0 {
        return Err(invalid(
            "circular-hole radius must be finite and positive in metres",
        ));
    }
    if !tolerance_m.is_finite() || tolerance_m <= 0.0 {
        return Err(invalid(
            "circular-hole tolerance must be finite and positive in metres",
        ));
    }
    let required_clearance = circle_radius_m + tolerance_m;
    if !required_clearance.is_finite() {
        return Err(invalid(
            "circular-hole radius plus tolerance must remain finite",
        ));
    }
    for axis in 0..2 {
        let [lower, upper] = bounds[axis];
        if lower >= upper {
            return Err(invalid(
                "circular-hole rectangle bounds must increase strictly",
            ));
        }
        if !(upper - lower).is_finite() {
            return Err(invalid("circular-hole rectangle span must remain finite"));
        }
        let lower_distance = circle_center[axis] - lower;
        let upper_distance = upper - circle_center[axis];
        if !lower_distance.is_finite() || !upper_distance.is_finite() {
            return Err(invalid(
                "circular-hole clearance arithmetic must remain finite",
            ));
        }
        if lower_distance <= required_clearance || upper_distance <= required_clearance {
            return Err(invalid(
                "closed circle must lie inside every rectangle side by more than the classification tolerance",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCircularHoleGeometryV1 {
    schema: String,
    encoding: String,
    kind: WireCircularHoleKind,
    length_unit: WireLengthUnit,
    tolerance_m: f64,
    bounds: [[f64; 2]; 2],
    circle: WireCircleV1,
    entity_sets: Vec<WireEntitySet>,
}

impl WireCircularHoleGeometryV1 {
    fn from_parts(
        bounds: [[f64; 2]; 2],
        circle_center: [f64; 2],
        circle_radius_m: f64,
        entity_sets: &[NamedEntitySet],
        tolerance_m: f64,
    ) -> Self {
        Self {
            schema: CIRCULAR_HOLE_SCHEMA.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            kind: WireCircularHoleKind::AxisAlignedRectangleWithCircularHoleV1,
            length_unit: WireLengthUnit::Metre,
            tolerance_m,
            bounds,
            circle: WireCircleV1 {
                center: circle_center,
                radius_m: circle_radius_m,
            },
            entity_sets: entity_sets.iter().map(WireEntitySet::from_set).collect(),
        }
    }

    fn check_contract(&self) -> Result<(), Diagnostic> {
        if self.schema != CIRCULAR_HOLE_SCHEMA
            || self.encoding != CANONICAL_ENCODING
            || self.kind != WireCircularHoleKind::AxisAlignedRectangleWithCircularHoleV1
            || self.length_unit != WireLengthUnit::Metre
        {
            return Err(invalid(
                "unsupported circular-hole geometry schema, encoding, kind, or unit",
            ));
        }
        Ok(())
    }

    fn check_limits(&self, limits: CanonicalGeometryLimits) -> Result<(), Diagnostic> {
        if CORNER_COUNT > limits.max_vertices {
            return Err(invalid(
                "circular-hole geometry vertex count exceeds decoder limits",
            ));
        }
        if FACE_COUNT > limits.max_faces {
            return Err(invalid(
                "circular-hole geometry face count exceeds decoder limits",
            ));
        }
        if OUTER_LOOP_INDEX_COUNT > limits.max_loop_indices {
            return Err(invalid(
                "circular-hole geometry loop-index count exceeds decoder limits",
            ));
        }
        if self.entity_sets.len() > limits.max_entity_sets {
            return Err(invalid(
                "circular-hole geometry entity-set count exceeds decoder limits",
            ));
        }
        let members = self
            .entity_sets
            .iter()
            .try_fold(0_usize, |total, set| total.checked_add(set.members.len()));
        if members.is_none_or(|count| count > limits.max_entity_set_members) {
            return Err(invalid(
                "circular-hole geometry entity-set member count exceeds decoder limits",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireCircularHoleKind {
    AxisAlignedRectangleWithCircularHoleV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCircleV1 {
    center: [f64; 2],
    radius_m: f64,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::CanonicalGeometryRef;

    const EXPECTED_DIGEST: [u8; 32] = [
        0xb0, 0x01, 0x23, 0x47, 0x2a, 0x59, 0x6e, 0x82, 0x89, 0x82, 0x0c, 0xab, 0xae, 0xe2, 0x0d,
        0x52, 0xcd, 0xf8, 0x1b, 0x55, 0x72, 0xfa, 0x9c, 0xe5, 0x8f, 0xf1, 0x7c, 0xda, 0xa0, 0x00,
        0x46, 0xd9,
    ];

    fn witness_with(bounds: [[f64; 2]; 2], center: [f64; 2]) -> CircularHoleGeometry {
        CircularHoleGeometry::new(
            bounds,
            center,
            0.05,
            vec![
                NamedEntitySet::new("fluid", 2, vec![0, 0]),
                NamedEntitySet::new("walls", 1, vec![3, 2]),
                NamedEntitySet::new("inlet", 1, vec![0]),
                NamedEntitySet::new("cylinder", 1, vec![4]),
                NamedEntitySet::new("outlet", 1, vec![1]),
            ],
            1e-12,
        )
        .expect("valid DFG-shaped geometry")
    }

    fn witness() -> CircularHoleGeometry {
        witness_with([[0.0, 2.2], [0.0, 0.41]], [0.2, 0.2])
    }

    #[test]
    pub(crate) fn independent_identity_witness_is_exact() {
        let geometry = witness();
        assert_eq!(geometry.canonical_bytes().len(), 511);
        assert_eq!(geometry.digest_bytes(), EXPECTED_DIGEST);
        assert_eq!(geometry.bounds(), &[[0.0, 2.2], [0.0, 0.41]]);
        assert_eq!(geometry.circle_center(), [0.2, 0.2]);
        assert_eq!(geometry.circle_radius_m(), 0.05);
        assert_eq!(
            geometry
                .entity_sets()
                .iter()
                .map(|set| (set.name(), set.dimension(), set.members()))
                .collect::<Vec<_>>(),
            vec![
                ("cylinder", 1, [4].as_slice()),
                ("inlet", 1, [0].as_slice()),
                ("outlet", 1, [1].as_slice()),
                ("walls", 1, [2, 3].as_slice()),
                ("fluid", 2, [0].as_slice()),
            ]
        );
        let common = crate::CanonicalGeometryV1::decode_circular_hole_canonical(
            geometry.canonical_bytes(),
            CanonicalGeometryLimits::default(),
        )
        .expect("accepted circular wire replays through the common owner");
        let reference = CanonicalGeometryRef::from(&common);
        assert_eq!(reference.digest_bytes(), EXPECTED_DIGEST);
        assert_eq!(reference.ambient_dimension(), 2);
        assert_eq!(reference.topological_dimension(), 2);
        assert_eq!(reference.entity_set_dimension("cylinder"), Some(1));
        assert_eq!(reference.entity_set_dimension("fluid"), Some(2));
    }

    #[test]
    fn canonical_decode_replays_and_rejects_other_spellings() {
        let geometry = witness();
        let decoded = CircularHoleGeometry::decode_canonical(
            geometry.canonical_bytes(),
            CanonicalGeometryLimits::default(),
        )
        .expect("canonical bytes replay");
        assert_eq!(decoded, geometry);

        let expanded_radius = String::from_utf8(geometry.canonical_bytes().to_vec())
            .expect("UTF-8")
            .replace("\"radius_m\":0.05", "\"radius_m\":0.050");
        let error = CircularHoleGeometry::decode_canonical(
            expanded_radius.as_bytes(),
            CanonicalGeometryLimits::default(),
        )
        .expect_err("equivalent noncanonical number spelling must fail");
        assert!(error.message().contains("not the canonical encoding"));

        let unknown = String::from_utf8(geometry.canonical_bytes().to_vec())
            .expect("UTF-8")
            .replace("\"entity_sets\":", "\"unknown\":0,\"entity_sets\":");
        assert!(
            CircularHoleGeometry::decode_canonical(
                unknown.as_bytes(),
                CanonicalGeometryLimits::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn signed_zero_has_one_identity() {
        let positive = witness();
        let negative = witness_with([[-0.0, 2.2], [-0.0, 0.41]], [0.2, 0.2]);
        assert_eq!(negative.canonical_bytes(), positive.canonical_bytes());
        assert_eq!(negative.digest_bytes(), positive.digest_bytes());
    }

    #[test]
    fn invalid_geometry_fails_closed() {
        let sets = || vec![NamedEntitySet::new("fluid", 2, vec![0])];
        for (bounds, center, radius, tolerance) in [
            ([[0.0, 0.0], [0.0, 1.0]], [0.5, 0.5], 0.1, 1e-12),
            ([[1.0, 0.0], [0.0, 1.0]], [0.5, 0.5], 0.1, 1e-12),
            ([[0.0, 1.0], [0.0, 1.0]], [0.5, 0.5], 0.0, 1e-12),
            ([[0.0, 1.0], [0.0, 1.0]], [0.5, 0.5], f64::NAN, 1e-12),
            ([[0.0, 1.0], [0.0, 1.0]], [0.5, 0.5], 0.1, 0.0),
            ([[0.0, 1.0], [0.0, 1.0]], [0.1, 0.5], 0.1, 1e-12),
            ([[0.0, 1.0], [0.0, 1.0]], [0.1875, 0.5], 0.125, 0.0625),
            (
                [[f64::NEG_INFINITY, 1.0], [0.0, 1.0]],
                [0.5, 0.5],
                0.1,
                1e-12,
            ),
            ([[-f64::MAX, f64::MAX], [0.0, 1.0]], [0.0, 0.5], 0.1, 1e-12),
        ] {
            assert!(CircularHoleGeometry::new(bounds, center, radius, sets(), tolerance,).is_err());
        }
    }

    #[test]
    fn entity_sets_and_decoder_budgets_fail_closed() {
        assert!(
            CircularHoleGeometry::new(
                [[0.0, 1.0], [0.0, 1.0]],
                [0.5, 0.5],
                0.1,
                vec![NamedEntitySet::new("bad", 1, vec![5])],
                1e-12,
            )
            .is_err()
        );
        let geometry = witness();
        let limits = CanonicalGeometryLimits {
            max_bytes: geometry.canonical_bytes().len() - 1,
            ..CanonicalGeometryLimits::default()
        };
        assert!(
            CircularHoleGeometry::decode_canonical(geometry.canonical_bytes(), limits,).is_err()
        );
        let limits = CanonicalGeometryLimits {
            max_entity_sets: 4,
            ..CanonicalGeometryLimits::default()
        };
        assert!(
            CircularHoleGeometry::decode_canonical(geometry.canonical_bytes(), limits,).is_err()
        );
        for limits in [
            CanonicalGeometryLimits {
                max_vertices: CORNER_COUNT - 1,
                ..CanonicalGeometryLimits::default()
            },
            CanonicalGeometryLimits {
                max_faces: FACE_COUNT - 1,
                ..CanonicalGeometryLimits::default()
            },
            CanonicalGeometryLimits {
                max_loop_indices: OUTER_LOOP_INDEX_COUNT - 1,
                ..CanonicalGeometryLimits::default()
            },
        ] {
            assert!(
                CircularHoleGeometry::decode_canonical(geometry.canonical_bytes(), limits,)
                    .is_err()
            );
        }
        let limits = CanonicalGeometryLimits {
            max_entity_set_members: 5,
            ..CanonicalGeometryLimits::default()
        };
        assert!(
            CircularHoleGeometry::decode_canonical(geometry.canonical_bytes(), limits,).is_err()
        );
    }
}
