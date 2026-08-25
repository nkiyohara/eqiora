//! Scale-independent canonical geometry derived from admitted construction topology.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::canonical::{CANONICAL_ENCODING, WireEntitySet, WireLengthUnit, digest_with_schema};
use crate::region::canonical_entity_sets;
use crate::{CanonicalGeometryLimits, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet};

pub(crate) const CONSTRUCTION_GEOMETRY_SCHEMA_V2: &str =
    "eqiora.planar-construction-circular-hole-envelope/v2";
const CORNER_COUNT: usize = 4;
const OUTER_LOOP_INDEX_COUNT: usize = 4;
const BOUNDARY_COUNT: usize = 5;
const FACE_COUNT: usize = 1;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Closed canonical rectangle-minus-circle geometry derived from construction lineage.
///
/// Unlike the frozen v1 circular-hole family, this contract contains no
/// classification tolerance. Its named entity sets are admitted from complete
/// result-topology handles by [`crate::CadAuthoredResultTopology`].
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalGeometryV2 {
    bounds: [[f64; 2]; 2],
    circle_center: [f64; 2],
    circle_radius_m: f64,
    entity_sets: Vec<NamedEntitySet>,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CanonicalGeometryV2 {
    pub(crate) fn new(
        mut bounds: [[f64; 2]; 2],
        mut circle_center: [f64; 2],
        circle_radius_m: f64,
        entity_sets: Vec<NamedEntitySet>,
    ) -> Result<Self, Diagnostic> {
        normalize_zeros(&mut bounds, &mut circle_center);
        validate_geometry(&bounds, &circle_center, circle_radius_m)?;
        let entity_sets =
            canonical_entity_sets(entity_sets, CORNER_COUNT, BOUNDARY_COUNT, FACE_COUNT)?;
        validate_complete_result_membership(&entity_sets)?;
        let wire = WireConstructionGeometryV2::from_parts(
            bounds,
            circle_center,
            circle_radius_m,
            &entity_sets,
        );
        let bytes = serde_json::to_vec(&wire).map_err(|error| {
            invalid(format!(
                "cannot serialize canonical construction geometry v2: {error}"
            ))
        })?;
        Ok(Self {
            bounds,
            circle_center,
            circle_radius_m,
            entity_sets,
            digest: digest_with_schema(CONSTRUCTION_GEOMETRY_SCHEMA_V2, &bytes),
            bytes,
        })
    }

    /// Decode one bounded byte-for-byte canonical v2 geometry.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed or unknown data, resource excess,
    /// invalid geometry or membership, or noncanonical JSON.
    pub fn decode_canonical(
        bytes: &[u8],
        limits: CanonicalGeometryLimits,
    ) -> Result<Self, Diagnostic> {
        if bytes.len() > limits.max_bytes {
            return Err(invalid(format!(
                "construction geometry v2 has {} bytes, exceeding the {} byte decoder limit",
                bytes.len(),
                limits.max_bytes
            )));
        }
        let wire: WireConstructionGeometryV2 = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid construction geometry v2 JSON: {error}")))?;
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
        )?;
        if canonical.canonical_bytes() != bytes {
            return Err(invalid(
                "construction geometry v2 JSON is not the canonical encoding of its content",
            ));
        }
        Ok(canonical)
    }

    /// Exact Cartesian bounds, ordered x then y as `[lower, upper]` metres.
    #[must_use]
    pub const fn bounds(&self) -> &[[f64; 2]; 2] {
        &self.bounds
    }

    /// Exact circle centre in metres.
    #[must_use]
    pub const fn circle_center(&self) -> [f64; 2] {
        self.circle_center
    }

    /// Exact positive circle radius in metres.
    #[must_use]
    pub const fn circle_radius_m(&self) -> f64 {
        self.circle_radius_m
    }

    /// Canonically ordered exact named entity sets.
    #[must_use]
    pub fn entity_sets(&self) -> &[NamedEntitySet] {
        &self.entity_sets
    }

    /// One exact named entity set.
    #[must_use]
    pub fn entity_set(&self, name: &str) -> Option<&NamedEntitySet> {
        self.entity_sets
            .iter()
            .find(|candidate| candidate.name() == name)
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

fn validate_complete_result_membership(entity_sets: &[NamedEntitySet]) -> Result<(), Diagnostic> {
    let mut covered = BTreeSet::new();
    for set in entity_sets {
        if set.dimension() != EDGE_DIMENSION && set.dimension() != FACE_DIMENSION {
            return Err(invalid(
                "construction geometry names may contain only result edges or faces",
            ));
        }
        for &member in set.members() {
            if !covered.insert((set.dimension(), member)) {
                return Err(invalid(
                    "construction geometry result membership must be named exactly once",
                ));
            }
        }
    }
    let expected = BTreeSet::from([
        (EDGE_DIMENSION, 0),
        (EDGE_DIMENSION, 1),
        (EDGE_DIMENSION, 2),
        (EDGE_DIMENSION, 3),
        (EDGE_DIMENSION, 4),
        (FACE_DIMENSION, 0),
    ]);
    if covered != expected {
        return Err(invalid(
            "construction geometry must name complete planar result membership exactly once",
        ));
    }
    Ok(())
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
) -> Result<(), Diagnostic> {
    if bounds.iter().flatten().any(|value| !value.is_finite())
        || circle_center.iter().any(|value| !value.is_finite())
    {
        return Err(invalid(
            "construction geometry coordinates and bounds must be finite metres",
        ));
    }
    if !circle_radius_m.is_finite() || circle_radius_m <= 0.0 {
        return Err(invalid(
            "construction geometry radius must be finite and positive in metres",
        ));
    }
    for axis in 0..2 {
        let [lower, upper] = bounds[axis];
        if lower >= upper {
            return Err(invalid(
                "construction geometry rectangle bounds must increase strictly",
            ));
        }
        if !(upper - lower).is_finite() {
            return Err(invalid(
                "construction geometry rectangle span must remain finite",
            ));
        }
        let lower_distance = circle_center[axis] - lower;
        let upper_distance = upper - circle_center[axis];
        if !lower_distance.is_finite() || !upper_distance.is_finite() {
            return Err(invalid(
                "construction geometry clearance arithmetic must remain finite",
            ));
        }
        if lower_distance <= circle_radius_m || upper_distance <= circle_radius_m {
            return Err(invalid(
                "closed construction circle must have strict positive clearance from every rectangle side",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireConstructionGeometryV2 {
    schema: String,
    encoding: String,
    kind: WireConstructionGeometryKindV2,
    length_unit: WireLengthUnit,
    bounds: [[f64; 2]; 2],
    circle: WireCircleV2,
    entity_sets: Vec<WireEntitySet>,
}

impl WireConstructionGeometryV2 {
    fn from_parts(
        bounds: [[f64; 2]; 2],
        circle_center: [f64; 2],
        circle_radius_m: f64,
        entity_sets: &[NamedEntitySet],
    ) -> Self {
        Self {
            schema: CONSTRUCTION_GEOMETRY_SCHEMA_V2.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            kind: WireConstructionGeometryKindV2::ConstructionProvenRectangleWithCircularHoleV2,
            length_unit: WireLengthUnit::Metre,
            bounds,
            circle: WireCircleV2 {
                center: circle_center,
                radius_m: circle_radius_m,
            },
            entity_sets: entity_sets.iter().map(WireEntitySet::from_set).collect(),
        }
    }

    fn check_contract(&self) -> Result<(), Diagnostic> {
        if self.schema != CONSTRUCTION_GEOMETRY_SCHEMA_V2
            || self.encoding != CANONICAL_ENCODING
            || self.kind
                != WireConstructionGeometryKindV2::ConstructionProvenRectangleWithCircularHoleV2
            || self.length_unit != WireLengthUnit::Metre
        {
            return Err(invalid(
                "unsupported construction geometry v2 schema, encoding, kind, or unit",
            ));
        }
        Ok(())
    }

    fn check_limits(&self, limits: CanonicalGeometryLimits) -> Result<(), Diagnostic> {
        if CORNER_COUNT > limits.max_vertices {
            return Err(invalid(
                "construction geometry vertex count exceeds decoder limits",
            ));
        }
        if FACE_COUNT > limits.max_faces {
            return Err(invalid(
                "construction geometry face count exceeds decoder limits",
            ));
        }
        if OUTER_LOOP_INDEX_COUNT > limits.max_loop_indices {
            return Err(invalid(
                "construction geometry loop-index count exceeds decoder limits",
            ));
        }
        if self.entity_sets.len() > limits.max_entity_sets {
            return Err(invalid(
                "construction geometry entity-set count exceeds decoder limits",
            ));
        }
        let members = self
            .entity_sets
            .iter()
            .try_fold(0_usize, |total, set| total.checked_add(set.members.len()));
        if members.is_none_or(|count| count > limits.max_entity_set_members) {
            return Err(invalid(
                "construction geometry entity-set member count exceeds decoder limits",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireConstructionGeometryKindV2 {
    ConstructionProvenRectangleWithCircularHoleV2,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCircleV2 {
    center: [f64; 2],
    radius_m: f64,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::process::Command;

    use sha2::{Digest, Sha256};

    use crate::{CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION};

    fn sets() -> Vec<NamedEntitySet> {
        vec![
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, vec![3, 2]),
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![1]),
        ]
    }

    #[test]
    pub(crate) fn strict_scale_independent_geometry_replays_without_tolerance() {
        for exponent in [-40, 0, 40] {
            let scale = 2.0_f64.powi(exponent);
            let geometry = CanonicalGeometryV2::new(
                [[0.0, 2.2 * scale], [0.0, 0.41 * scale]],
                [0.2 * scale, 0.2 * scale],
                0.05 * scale,
                sets(),
            )
            .unwrap();
            assert!(
                !geometry
                    .canonical_bytes()
                    .windows(b"tolerance_m".len())
                    .any(|window| window == b"tolerance_m")
            );
            assert_eq!(
                CanonicalGeometryV2::decode_canonical(
                    geometry.canonical_bytes(),
                    CanonicalGeometryLimits::default(),
                )
                .unwrap(),
                geometry
            );
        }
    }

    #[test]
    pub(crate) fn independent_oracle_freezes_exact_v2_artifact() {
        let oracle = Command::new("python3")
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../verify/geometry/construction-proven-planar-geometry-v2/oracle.py"),
            )
            .output()
            .expect("independent Python v2 identity oracle must execute");
        assert!(
            oracle.status.success(),
            "independent oracle failed: {}",
            String::from_utf8_lossy(&oracle.stderr)
        );
        let oracle_output = String::from_utf8(oracle.stdout).expect("oracle emits UTF-8");
        let oracle_wire = oracle_output
            .lines()
            .next()
            .expect("oracle emits complete canonical JSON")
            .as_bytes();
        assert!(oracle_output.contains("bytes=511"));
        assert!(
            oracle_output.contains(
                "sha256=1811037532ef5697a2c331d47786d39b2a0d3a64b2f348e7859342e742fecca0"
            )
        );
        assert!(oracle_output.contains(
            "plain_sha256=bdcd32d3829ad1bf7b8ef455a09bdbe863db88dc6454584381ef38421ea29ddc"
        ));

        let geometry =
            CanonicalGeometryV2::new([[0.0, 2.2], [0.0, 0.41]], [0.2, 0.2], 0.05, sets()).unwrap();
        assert_eq!(geometry.canonical_bytes(), oracle_wire);
        assert_eq!(geometry.canonical_bytes().len(), 511);
        assert_eq!(
            hex(geometry.digest_bytes()),
            "1811037532ef5697a2c331d47786d39b2a0d3a64b2f348e7859342e742fecca0"
        );
        let plain_digest: [u8; 32] = Sha256::digest(oracle_wire).into();
        assert_eq!(
            hex(plain_digest),
            "bdcd32d3829ad1bf7b8ef455a09bdbe863db88dc6454584381ef38421ea29ddc"
        );
        assert_ne!(plain_digest, geometry.digest_bytes());
        assert_eq!(
            CanonicalGeometryV2::decode_canonical(oracle_wire, Default::default()).unwrap(),
            geometry
        );

        let signed_zero =
            CanonicalGeometryV2::new([[-0.0, 2.2], [-0.0, 0.41]], [0.2, 0.2], 0.05, sets())
                .unwrap();
        assert_eq!(signed_zero.canonical_bytes(), oracle_wire);
        assert_eq!(signed_zero.digest_bytes(), geometry.digest_bytes());

        let v1 = CanonicalGeometryV1::from_circular_hole(
            [[0.0, 2.2], [0.0, 0.41]],
            [0.2, 0.2],
            0.05,
            sets(),
            1.0e-12,
        )
        .unwrap();
        assert_ne!(v1.canonical_bytes(), geometry.canonical_bytes());
        assert_ne!(v1.digest_bytes(), geometry.digest_bytes());
    }

    #[test]
    pub(crate) fn finite_increasing_positive_and_strict_clearance_predicates_fail_closed() {
        let valid_bounds = [[0.0, 2.2], [0.0, 0.41]];
        let valid_center = [0.2, 0.2];
        for (bounds, center, radius) in [
            ([[f64::NAN, 2.2], [0.0, 0.41]], valid_center, 0.05),
            ([[0.0, 0.0], [0.0, 0.41]], valid_center, 0.05),
            (valid_bounds, [f64::INFINITY, 0.2], 0.05),
            (valid_bounds, valid_center, 0.0),
            (valid_bounds, valid_center, f64::INFINITY),
            ([[0.0, 1.0], [0.0, 1.0]], [0.25, 0.5], 0.25),
            ([[0.0, 1.0], [0.0, 1.0]], [1.25, 0.5], 0.25),
        ] {
            assert!(CanonicalGeometryV2::new(bounds, center, radius, sets()).is_err());
        }

        let mut incomplete = sets();
        incomplete.retain(|set| set.name() != "cylinder");
        assert!(CanonicalGeometryV2::new(valid_bounds, valid_center, 0.05, incomplete).is_err());

        let mut duplicate = sets();
        duplicate.push(NamedEntitySet::new("second-inlet", EDGE_DIMENSION, vec![0]));
        assert!(CanonicalGeometryV2::new(valid_bounds, valid_center, 0.05, duplicate).is_err());
    }

    #[test]
    pub(crate) fn closed_v2_decoder_rejects_noncanonical_and_open_wire_mutants() {
        let geometry =
            CanonicalGeometryV2::new([[0.0, 2.2], [0.0, 0.41]], [0.2, 0.2], 0.05, sets()).unwrap();
        let wire = String::from_utf8(geometry.canonical_bytes().to_vec()).unwrap();
        let reordered = serde_json::to_vec(
            &serde_json::from_slice::<serde_json::Value>(geometry.canonical_bytes()).unwrap(),
        )
        .unwrap();
        for mutant in [
            wire.replacen('{', "{\"unknown\":0,", 1).into_bytes(),
            wire.replacen('{', "{\"tolerance_m\":1e-12,", 1)
                .into_bytes(),
            wire.replacen('{', "{\"classification_tolerance_m\":1e-12,", 1)
                .into_bytes(),
            wire.replacen(
                "\"schema\":",
                "\"schema\":\"eqiora.planar-construction-circular-hole-envelope/v2\",\"schema\":",
                1,
            )
            .into_bytes(),
            wire.replacen("\"members\":[2,3]", "\"members\":[3,2]", 1)
                .into_bytes(),
            wire.replacen("\"bounds\":[[0.0,", "\"bounds\":[[0,", 1)
                .into_bytes(),
            reordered,
        ] {
            assert_ne!(mutant, geometry.canonical_bytes());
            assert!(CanonicalGeometryV2::decode_canonical(&mutant, Default::default()).is_err());
        }
    }

    fn hex(bytes: [u8; 32]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
