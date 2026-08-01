//! Source-bound chordal reference mesh for one exact circular-hole geometry.
//!
//! This is a bounded reference realization, not a production mesher. The
//! exact circle remains geometry meaning; every sampled coordinate, segment
//! count, approximation metric, and mesh-quality policy belongs only to this
//! value.

use std::f64::consts::{PI, TAU};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};

use crate::circular_hole::CircularHoleGeometry;
use crate::{
    CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION, GeometryRevisionReference, NamedEntitySet,
    PlanarFace, PlanarRegion, VERTEX_DIMENSION,
};

const MINIMUM_SEGMENTS: usize = 8;
const MAXIMUM_REFERENCE_SEGMENTS: usize = 100_000;
const BINARY64_EVALUATION_ULPS: f64 = 128.0;
const RECTANGLE_CORNER_COUNT: usize = 4;
const EXACT_BOUNDARY_COUNT: usize = 5;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// One exact-source-bound, error-controlled chordal reference mesh.
///
/// Construction is the only safe way to bind the generated straight-edged
/// region and affine-triangle mesh to the exact circular-hole source. This
/// value has no durable wire. Its fixed radial-fan topology is intended for
/// conformance and bounded demonstrations; passing its quality gate is not a
/// production mesh-quality claim.
#[derive(Clone, Debug, PartialEq)]
pub struct CircularHoleChordalMeshV1 {
    source: GeometryRevisionReference,
    requested_max_boundary_error_m: f64,
    boundary_evaluation_allowance_m: f64,
    boundary_error_bound_m: f64,
    circle_segments: usize,
    circle_area_deficit_m2: f64,
    circle_perimeter_deficit_m: f64,
    region: PlanarRegion,
    mesh: SimplicialMesh,
}

impl CircularHoleChordalMeshV1 {
    /// Derive one bounded chordal region and affine-triangle reference mesh.
    ///
    /// The circle uses a regular inscribed loop with phase
    /// `theta_i = 2 pi i / n`. A stable half-angle inverse selects the first
    /// candidate count, then the generated binary64 loop is measured directly.
    /// The accepted boundary bound includes a precommitted scale-aware
    /// binary64 evaluation allowance and never reuses source classification
    /// tolerance as approximation policy.
    ///
    /// Exact source entity sets expand to the realized rectangle corners,
    /// side edges, circular chords, and one face without accepting mesh labels.
    ///
    /// # Errors
    /// Returns `EQ0901` when the common owner is not the admitted exact
    /// circular-hole kind, or for an invalid error/work request,
    /// unrepresentable sampled geometry, failed exact-entity propagation, or
    /// a boundary bound that cannot be met within the work limit. Mesh
    /// topology and quality failures retain the `SimplicialMesh` `EQ0803`
    /// diagnostic.
    pub fn from_exact(
        source: &CanonicalGeometryV1,
        requested_max_boundary_error_m: f64,
        max_segments: usize,
        quality_gate: MeshQualityGate,
    ) -> Result<Self, Diagnostic> {
        let exact = source.circular_hole().ok_or_else(|| {
            invalid("chordal circular-hole realization requires exact circular-hole geometry")
        })?;
        validate_work_limit(max_segments)?;
        let evaluation_allowance_m = evaluation_allowance_m(exact)?;
        if !requested_max_boundary_error_m.is_finite()
            || requested_max_boundary_error_m <= evaluation_allowance_m
        {
            return Err(invalid(format!(
                "chordal circular-boundary error must be finite and greater than the \
                 binary64 evaluation allowance {evaluation_allowance_m} m",
            )));
        }

        let radius_m = exact.circle_radius_m();
        let effective_error_m = requested_max_boundary_error_m - evaluation_allowance_m;
        let mut circle_segments = stable_segment_count(radius_m, effective_error_m, max_segments)?;

        let (circle, measurements) = loop {
            let circle = sample_circle(exact, circle_segments)?;
            validate_circle_loop(exact.circle_center(), &circle)?;
            let measurements = measure_circle(exact.circle_center(), radius_m, &circle)?;
            let accepted_bound = measurements.boundary_deviation_m + evaluation_allowance_m;
            if !accepted_bound.is_finite() {
                return Err(invalid("chordal circular-boundary error bound overflows"));
            }
            if accepted_bound <= requested_max_boundary_error_m {
                break (circle, measurements);
            }
            circle_segments = circle_segments
                .checked_add(1)
                .ok_or_else(|| invalid("chordal circular-boundary segment correction overflows"))?;
            if circle_segments > max_segments {
                return Err(invalid(format!(
                    "chordal circular-boundary error requires more than the caller limit of \
                     {max_segments} segments",
                )));
            }
        };

        let boundary_error_bound_m = measurements.boundary_deviation_m + evaluation_allowance_m;
        let (vertices, cells, unnamed_region) =
            build_reference_topology(exact, circle_segments, circle)?;
        let region = attach_source_entity_sets(exact, &unnamed_region)?;
        let mesh = SimplicialMesh::new(
            2,
            vertices
                .into_iter()
                .map(|coordinate| coordinate.to_vec())
                .collect(),
            cells,
            quality_gate,
        )?;

        Ok(Self {
            source: GeometryRevisionReference::from_digest_bytes(source.digest_bytes()),
            requested_max_boundary_error_m,
            boundary_evaluation_allowance_m: evaluation_allowance_m,
            boundary_error_bound_m,
            circle_segments,
            circle_area_deficit_m2: measurements.area_deficit_m2,
            circle_perimeter_deficit_m: measurements.perimeter_deficit_m,
            region,
            mesh,
        })
    }

    /// Exact circular-hole geometry revision that this mesh realizes.
    #[must_use]
    pub const fn source(&self) -> GeometryRevisionReference {
        self.source
    }

    /// Caller-requested maximum symmetric circular-boundary error in metres.
    #[must_use]
    pub const fn requested_max_boundary_error_m(&self) -> f64 {
        self.requested_max_boundary_error_m
    }

    /// Precommitted scale-aware binary64 evaluation allowance in metres.
    #[must_use]
    pub const fn boundary_evaluation_allowance_m(&self) -> f64 {
        self.boundary_evaluation_allowance_m
    }

    /// Accepted measured circular-boundary error bound in metres.
    #[must_use]
    pub const fn boundary_error_bound_m(&self) -> f64 {
        self.boundary_error_bound_m
    }

    /// Number of straight segments realizing the circular boundary.
    #[must_use]
    pub const fn circle_segments(&self) -> usize {
        self.circle_segments
    }

    /// Measured exact-circle minus chordal-loop area in square metres.
    #[must_use]
    pub const fn circle_area_deficit_m2(&self) -> f64 {
        self.circle_area_deficit_m2
    }

    /// Measured exact-circle minus chordal-loop perimeter in metres.
    #[must_use]
    pub const fn circle_perimeter_deficit_m(&self) -> f64 {
        self.circle_perimeter_deficit_m
    }

    /// Canonical straight-edged region with source-derived named entity sets.
    ///
    /// Region vertex indices are independently canonicalized and are not the
    /// mesh's author-order vertex indices; use geometry/mesh correspondence
    /// rather than equating those index spaces.
    #[must_use]
    pub const fn region(&self) -> &PlanarRegion {
        &self.region
    }

    /// Validated affine-triangle reference mesh.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMesh {
        &self.mesh
    }
}

fn validate_work_limit(max_segments: usize) -> Result<(), Diagnostic> {
    if max_segments < MINIMUM_SEGMENTS {
        return Err(invalid(format!(
            "chordal circular-boundary work limit must admit at least \
             {MINIMUM_SEGMENTS} segments",
        )));
    }
    if max_segments > MAXIMUM_REFERENCE_SEGMENTS {
        return Err(invalid(format!(
            "chordal circular-boundary caller limit {max_segments} exceeds the \
             {MAXIMUM_REFERENCE_SEGMENTS}-segment reference-path hard limit",
        )));
    }
    Ok(())
}

fn evaluation_allowance_m(source: &CircularHoleGeometry) -> Result<f64, Diagnostic> {
    let scale_m = source
        .bounds()
        .iter()
        .flatten()
        .copied()
        .chain(source.circle_center())
        .chain(std::iter::once(source.circle_radius_m()))
        .map(f64::abs)
        .fold(f64::MIN_POSITIVE, f64::max);
    let allowance_m = BINARY64_EVALUATION_ULPS * f64::EPSILON * scale_m;
    if !allowance_m.is_finite() || allowance_m <= 0.0 {
        return Err(invalid(
            "chordal binary64 boundary-evaluation allowance must remain finite and positive",
        ));
    }
    Ok(allowance_m)
}

fn stable_segment_count(
    radius_m: f64,
    effective_error_m: f64,
    max_segments: usize,
) -> Result<usize, Diagnostic> {
    let mut candidate = analytic_segment_candidate(radius_m, effective_error_m)?;
    let maximum = u64::try_from(max_segments)
        .map_err(|_| invalid("chordal segment work limit exceeds portable u64"))?;
    while candidate > MINIMUM_SEGMENTS as u64
        && stable_sagitta_m(radius_m, candidate - 1) <= effective_error_m
    {
        candidate -= 1;
    }
    if candidate > maximum {
        return Err(invalid(format!(
            "chordal circular-boundary error requires {candidate} segments, exceeding the \
             caller limit of {max_segments}",
        )));
    }
    while stable_sagitta_m(radius_m, candidate) > effective_error_m {
        candidate = candidate
            .checked_add(1)
            .ok_or_else(|| invalid("chordal segment count correction overflows"))?;
        if candidate > maximum {
            return Err(invalid(format!(
                "chordal circular-boundary error requires more than the caller limit of \
                 {max_segments} segments",
            )));
        }
    }
    usize::try_from(candidate)
        .map_err(|_| invalid("chordal segment count exceeds the local usize range"))
}

fn analytic_segment_candidate(radius_m: f64, effective_error_m: f64) -> Result<u64, Diagnostic> {
    if !radius_m.is_finite()
        || radius_m <= 0.0
        || !effective_error_m.is_finite()
        || effective_error_m <= 0.0
    {
        return Err(invalid(
            "chordal segment selection requires finite positive radius and effective error",
        ));
    }
    if effective_error_m >= 2.0 * radius_m {
        return Ok(MINIMUM_SEGMENTS as u64);
    }
    let half_angle = (effective_error_m / (2.0 * radius_m)).sqrt().asin();
    let raw = (PI / (2.0 * half_angle)).ceil();
    if !raw.is_finite() || raw > u64::MAX as f64 {
        return Err(invalid(
            "chordal segment count exceeds the representable work range",
        ));
    }
    Ok((raw as u64).max(MINIMUM_SEGMENTS as u64))
}

fn stable_sagitta_m(radius_m: f64, segments: u64) -> f64 {
    let sine = (PI / (2.0 * segments as f64)).sin();
    2.0 * radius_m * sine * sine
}

fn sample_circle(
    source: &CircularHoleGeometry,
    segments: usize,
) -> Result<Vec<[f64; 2]>, Diagnostic> {
    let center = source.circle_center();
    let radius_m = source.circle_radius_m();
    let mut vertices = Vec::with_capacity(segments);
    for index in 0..segments {
        let theta = TAU * index as f64 / segments as f64;
        let vertex = [
            center[0] + radius_m * theta.cos(),
            center[1] + radius_m * theta.sin(),
        ];
        if vertex.iter().any(|coordinate| !coordinate.is_finite()) {
            return Err(invalid(
                "chordal circular-boundary coordinate construction is non-finite",
            ));
        }
        vertices.push(vertex);
    }
    Ok(vertices)
}

fn validate_circle_loop(center: [f64; 2], vertices: &[[f64; 2]]) -> Result<(), Diagnostic> {
    let mut winding_angle = 0.0;
    for index in 0..vertices.len() {
        let previous = relative(
            vertices[(index + vertices.len() - 1) % vertices.len()],
            center,
        );
        let start = relative(vertices[index], center);
        let end = relative(vertices[(index + 1) % vertices.len()], center);
        let radial_cross = cross(start, end);
        let center_side = cross(subtract(end, start), [-start[0], -start[1]]);
        let vertex_turn = cross(subtract(start, previous), subtract(end, start));
        if radial_cross <= 0.0 || center_side <= 0.0 || vertex_turn <= 0.0 {
            return Err(invalid(
                "sampled circular loop must be strictly convex, simple, and contain its centre",
            ));
        }
        winding_angle += radial_cross.atan2(dot(start, end));
    }
    let winding_tolerance = 64.0 * f64::EPSILON * vertices.len() as f64;
    if !winding_angle.is_finite() || (winding_angle - TAU).abs() > winding_tolerance {
        return Err(invalid(
            "sampled circular loop must wind exactly once around its centre",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct CircleMeasurements {
    boundary_deviation_m: f64,
    area_deficit_m2: f64,
    perimeter_deficit_m: f64,
}

type ReferenceTopologyBuild = (Vec<[f64; 2]>, Vec<Vec<usize>>, PlanarRegion);

fn measure_circle(
    center: [f64; 2],
    radius_m: f64,
    vertices: &[[f64; 2]],
) -> Result<CircleMeasurements, Diagnostic> {
    let mut minimum_edge_radius_m = f64::INFINITY;
    let mut maximum_vertex_radius_m: f64 = 0.0;
    let mut twice_area_m2 = 0.0;
    let mut perimeter_m = 0.0;
    for index in 0..vertices.len() {
        let start = relative(vertices[index], center);
        let end = relative(vertices[(index + 1) % vertices.len()], center);
        let edge = subtract(end, start);
        let edge_squared = dot(edge, edge);
        if !edge_squared.is_finite() || edge_squared <= 0.0 {
            return Err(invalid(
                "sampled circular loop contains a degenerate or non-finite chord",
            ));
        }
        let parameter = (-dot(start, edge) / edge_squared).clamp(0.0, 1.0);
        let closest = [
            start[0] + parameter * edge[0],
            start[1] + parameter * edge[1],
        ];
        minimum_edge_radius_m = minimum_edge_radius_m.min(closest[0].hypot(closest[1]));
        maximum_vertex_radius_m = maximum_vertex_radius_m.max(start[0].hypot(start[1]));
        twice_area_m2 += cross(start, end);
        perimeter_m += edge[0].hypot(edge[1]);
    }

    let boundary_deviation_m = (radius_m - minimum_edge_radius_m)
        .max(maximum_vertex_radius_m - radius_m)
        .max(0.0);
    let exact_area_m2 = PI * radius_m * radius_m;
    let area_deficit_m2 = exact_area_m2 - 0.5 * twice_area_m2.abs();
    let perimeter_deficit_m = TAU * radius_m - perimeter_m;
    if [boundary_deviation_m, area_deficit_m2, perimeter_deficit_m]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(invalid(
            "sampled circular-loop approximation metrics must remain finite and non-negative",
        ));
    }
    Ok(CircleMeasurements {
        boundary_deviation_m,
        area_deficit_m2,
        perimeter_deficit_m,
    })
}

fn build_reference_topology(
    source: &CircularHoleGeometry,
    segments: usize,
    circle: Vec<[f64; 2]>,
) -> Result<ReferenceTopologyBuild, Diagnostic> {
    let vertex_capacity = segments
        .checked_mul(2)
        .and_then(|count| count.checked_add(RECTANGLE_CORNER_COUNT))
        .ok_or_else(|| invalid("chordal reference vertex capacity overflows"))?;
    let cell_capacity = segments
        .checked_mul(2)
        .and_then(|count| count.checked_add(RECTANGLE_CORNER_COUNT))
        .ok_or_else(|| invalid("chordal reference cell capacity overflows"))?;
    let mut vertices = Vec::with_capacity(vertex_capacity);
    vertices.extend(circle);
    for index in 0..segments {
        let theta = TAU * index as f64 / segments as f64;
        vertices.push(ray_rectangle_intersection(source, theta)?);
    }

    let corners = exact_rectangle_corners(source);
    let mut corner_indices = Vec::with_capacity(RECTANGLE_CORNER_COUNT);
    for corner in corners {
        let candidates = (segments..(2 * segments))
            .filter(|&index| distance(vertices[index], corner) <= source.tolerance_m())
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => {
                corner_indices.push(vertices.len());
                vertices.push(corner);
            }
            [index] => {
                vertices[*index] = corner;
                corner_indices.push(*index);
            }
            _ => {
                return Err(invalid(
                    "more than one radial rectangle sample lies within source tolerance of one corner",
                ));
            }
        }
    }

    let mut outer_loop = Vec::with_capacity(segments + RECTANGLE_CORNER_COUNT);
    let mut cells = Vec::with_capacity(cell_capacity);
    for index in 0..segments {
        let next = (index + 1) % segments;
        let outer = segments + index;
        let outer_next = segments + next;
        cells.push(oriented_triangle([outer, outer_next, next], &vertices)?);
        cells.push(oriented_triangle([outer, next, index], &vertices)?);

        outer_loop.push(outer);
        let low = TAU * index as f64 / segments as f64;
        let high = TAU * (index + 1) as f64 / segments as f64;
        let mut crossed = corner_indices
            .iter()
            .copied()
            .filter(|corner| {
                if *corner == outer || *corner == outer_next {
                    return false;
                }
                let theta = positive_angle(vertices[*corner], source.circle_center());
                theta > low && theta < high
            })
            .collect::<Vec<_>>();
        crossed.sort_by(|left, right| {
            positive_angle(vertices[*left], source.circle_center())
                .total_cmp(&positive_angle(vertices[*right], source.circle_center()))
        });
        outer_loop.extend(crossed.iter().copied());
        crossed.push(outer_next);
        for pair in crossed.windows(2) {
            cells.push(oriented_triangle([outer, pair[0], pair[1]], &vertices)?);
        }
    }

    let unnamed_region = PlanarRegion::new(
        vertices.clone(),
        vec![PlanarFace::new(outer_loop, vec![(0..segments).collect()])],
        Vec::new(),
        source.tolerance_m(),
    )?;
    Ok((vertices, cells, unnamed_region))
}

fn ray_rectangle_intersection(
    source: &CircularHoleGeometry,
    theta: f64,
) -> Result<[f64; 2], Diagnostic> {
    let center = source.circle_center();
    let bounds = source.bounds();
    let direction = [theta.cos(), theta.sin()];
    let x_bound = if direction[0] > 0.0 {
        bounds[0][1]
    } else {
        bounds[0][0]
    };
    let y_bound = if direction[1] > 0.0 {
        bounds[1][1]
    } else {
        bounds[1][0]
    };
    let x_time = if direction[0] == 0.0 {
        f64::INFINITY
    } else {
        (x_bound - center[0]) / direction[0]
    };
    let y_time = if direction[1] == 0.0 {
        f64::INFINITY
    } else {
        (y_bound - center[1]) / direction[1]
    };
    let hit = if x_time < y_time {
        [x_bound, center[1] + x_time * direction[1]]
    } else {
        [center[0] + y_time * direction[0], y_bound]
    };
    if hit.iter().any(|coordinate| !coordinate.is_finite()) {
        return Err(invalid("radial rectangle intersection must remain finite"));
    }
    Ok(hit)
}

fn attach_source_entity_sets(
    source: &CircularHoleGeometry,
    unnamed: &PlanarRegion,
) -> Result<PlanarRegion, Diagnostic> {
    let corners = exact_rectangle_corners(source);
    let corner_members = corners
        .iter()
        .map(|corner| {
            unnamed
                .vertices()
                .iter()
                .position(|candidate| candidate == corner)
                .ok_or_else(|| invalid("chordal region is missing one exact rectangle corner"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let face = unnamed
        .faces()
        .first()
        .ok_or_else(|| invalid("chordal region is missing its one face"))?;
    if unnamed.faces().len() != 1 || face.holes().len() != 1 {
        return Err(invalid(
            "chordal circular-hole region must contain exactly one face and one hole",
        ));
    }
    let mut boundary_members: [Vec<usize>; EXACT_BOUNDARY_COUNT] =
        std::array::from_fn(|_| Vec::new());
    let mut edge_index = 0;
    for position in 0..face.outer().len() {
        let start = unnamed.vertices()[face.outer()[position]];
        let end = unnamed.vertices()[face.outer()[(position + 1) % face.outer().len()]];
        let side = rectangle_side(source, start, end).ok_or_else(|| {
            invalid("one chordal outer edge does not lie on an exact rectangle side")
        })?;
        boundary_members[side].push(edge_index);
        edge_index += 1;
    }
    for hole in face.holes() {
        for _ in hole {
            boundary_members[4].push(edge_index);
            edge_index += 1;
        }
    }
    if boundary_members.iter().any(Vec::is_empty) || edge_index != unnamed.edge_count() {
        return Err(invalid(
            "chordal region does not realize every exact circular-hole boundary",
        ));
    }

    let mut entity_sets = Vec::with_capacity(source.entity_sets().len());
    for source_set in source.entity_sets() {
        let mut members = Vec::new();
        for &member in source_set.members() {
            match source_set.dimension() {
                VERTEX_DIMENSION => members.push(corner_members[member]),
                EDGE_DIMENSION => members.extend(boundary_members[member].iter().copied()),
                FACE_DIMENSION => members.push(0),
                _ => {
                    return Err(invalid(
                        "exact circular-hole entity set has an unsupported dimension",
                    ));
                }
            }
        }
        entity_sets.push(NamedEntitySet::new(
            source_set.name(),
            source_set.dimension(),
            members,
        ));
    }
    PlanarRegion::new(
        unnamed.vertices().to_vec(),
        unnamed.faces().to_vec(),
        entity_sets,
        source.tolerance_m(),
    )
}

fn exact_rectangle_corners(source: &CircularHoleGeometry) -> [[f64; 2]; 4] {
    let bounds = source.bounds();
    [
        [bounds[0][0], bounds[1][0]],
        [bounds[0][0], bounds[1][1]],
        [bounds[0][1], bounds[1][0]],
        [bounds[0][1], bounds[1][1]],
    ]
}

fn rectangle_side(source: &CircularHoleGeometry, start: [f64; 2], end: [f64; 2]) -> Option<usize> {
    let bounds = source.bounds();
    if start[0] == bounds[0][0] && end[0] == bounds[0][0] {
        Some(0)
    } else if start[0] == bounds[0][1] && end[0] == bounds[0][1] {
        Some(1)
    } else if start[1] == bounds[1][0] && end[1] == bounds[1][0] {
        Some(2)
    } else if start[1] == bounds[1][1] && end[1] == bounds[1][1] {
        Some(3)
    } else {
        None
    }
}

fn oriented_triangle(
    mut cell: [usize; 3],
    vertices: &[[f64; 2]],
) -> Result<Vec<usize>, Diagnostic> {
    let start = vertices[cell[0]];
    let first = subtract(vertices[cell[1]], start);
    let second = subtract(vertices[cell[2]], start);
    let orientation = cross(first, second);
    if !orientation.is_finite() || orientation == 0.0 {
        return Err(invalid(
            "chordal reference topology contains a degenerate triangle",
        ));
    }
    if orientation < 0.0 {
        cell.swap(1, 2);
    }
    Ok(cell.to_vec())
}

fn positive_angle(point: [f64; 2], center: [f64; 2]) -> f64 {
    let mut angle = (point[1] - center[1]).atan2(point[0] - center[0]);
    if angle <= 0.0 {
        angle += TAU;
    }
    angle
}

fn relative(point: [f64; 2], center: [f64; 2]) -> [f64; 2] {
    [point[0] - center[0], point[1] - center[1]]
}

fn subtract(left: [f64; 2], right: [f64; 2]) -> [f64; 2] {
    [left[0] - right[0], left[1] - right[1]]
}

fn dot(left: [f64; 2], right: [f64; 2]) -> f64 {
    left[0].mul_add(right[0], left[1] * right[1])
}

fn cross(left: [f64; 2], right: [f64; 2]) -> f64 {
    left[0].mul_add(right[1], -(left[1] * right[0]))
}

fn distance(left: [f64; 2], right: [f64; 2]) -> f64 {
    (left[0] - right[0]).hypot(left[1] - right[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_inverse_survives_the_frozen_deep_cancellation_case() {
        assert_eq!(
            analytic_segment_candidate(0.05, 1.0e-18).unwrap(),
            496_729_414
        );
        assert_eq!(1.0 - 1.0e-18 / 0.05, 1.0);
    }

    #[test]
    fn large_effective_error_branches_before_inverse_domain_failure() {
        assert_eq!(analytic_segment_candidate(0.05, 0.2).unwrap(), 8);
    }

    #[test]
    fn corrected_candidate_is_bounded_only_after_the_direct_predicate() {
        let error = stable_sagitta_m(0.05, 50);
        assert_eq!(stable_segment_count(0.05, error, 50).unwrap(), 50);
    }

    #[test]
    fn positive_radial_crossings_do_not_admit_a_double_winding_star() {
        let order = [0, 2, 4, 1, 3];
        let vertices = order
            .into_iter()
            .map(|index| {
                let theta = TAU * index as f64 / 5.0;
                [theta.cos(), theta.sin()]
            })
            .collect::<Vec<_>>();
        assert!(validate_circle_loop([0.0, 0.0], &vertices).is_err());
    }
}
