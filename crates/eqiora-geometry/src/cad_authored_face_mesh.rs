//! Source-bound reference mesh for one rectangular authored CAD face.
//!
//! The mesh remains intrinsically two-dimensional. Its placement on the
//! selected three-dimensional face is retained separately, so downstream
//! consumers never infer a frame from global axes or mesh coordinates.

use std::f64::consts::SQRT_2;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};

use crate::{CadAuthoredFaceHandle, CadAuthoredGraph, PlanarFace, PlanarRegion};

const MINIMUM_TRIANGLES: usize = 2;
const MAXIMUM_REFERENCE_TRIANGLES: usize = 100_000;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// One graph-bound rectangular face realized as an affine-triangle mesh.
///
/// This value has no durable wire. It keeps the exact authored source binding,
/// the independently supplied Geometry classification tolerance and sizing
/// request, the face placement, and the accepted intrinsic region and mesh in
/// one owner. Artifact envelopes remain the durable Geometry and Mesh outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct CadAuthoredFaceMesh {
    source_graph_digest: [u8; 32],
    source_face: CadAuthoredFaceHandle,
    geometry_classification_tolerance_m: f64,
    target_maximum_edge_length_m: f64,
    maximum_triangles: usize,
    origin_m: [f64; 3],
    u_hat: [f64; 3],
    v_hat: [f64; 3],
    parent_outward_normal: [f64; 3],
    u_length_m: f64,
    v_length_m: f64,
    u_divisions: usize,
    v_divisions: usize,
    u_maximum_realized_gap_m: f64,
    v_maximum_realized_gap_m: f64,
    region: PlanarRegion,
    mesh: SimplicialMesh,
}

impl CadAuthoredFaceMesh {
    /// Realize one admitted rectangular face under a bounded local request.
    ///
    /// Per-axis subdivision is the least positive integer `n` whose generated
    /// endpoint-snapped binary64 coordinates have maximum adjacent gap `D`
    /// satisfying `D.hypot(D) <= target`. The Geometry classification
    /// tolerance rejects degenerate face axes and becomes part of the intrinsic
    /// region identity; it never changes sizing. This reference path admits
    /// only the current provider's exactly closed, exactly orthogonal rectangle
    /// cycles. Approximate or arbitrarily rotated CAD frames remain unsupported.
    ///
    /// # Errors
    /// Returns `EQ0901` before topology allocation for a stale or foreign
    /// handle, a non-rectangular face, an invalid scalar or work limit, an
    /// unrepresentable frame, or a resolved mesh exceeding the caller budget.
    /// Accepted topology is then subject to the unchanged `SimplicialMesh`
    /// `EQ0803` orientation and quality gate.
    pub fn from_face(
        graph: &CadAuthoredGraph,
        source_face: &CadAuthoredFaceHandle,
        geometry_classification_tolerance_m: f64,
        target_maximum_edge_length_m: f64,
        maximum_triangles: usize,
        quality_gate: MeshQualityGate,
    ) -> Result<Self, Diagnostic> {
        validate_request(
            geometry_classification_tolerance_m,
            target_maximum_edge_length_m,
            maximum_triangles,
        )?;

        let cycle = graph
            .rectangular_face_vertices_m(source_face)?
            .ok_or_else(|| invalid("authored CAD face is not a supported rectangle"))?;
        let parent_outward_normal = graph
            .planar_face_outward_normal(source_face)?
            .ok_or_else(|| invalid("authored CAD face has no planar outward normal"))?;
        let frame = FaceFrame::from_cycle(
            cycle,
            parent_outward_normal,
            geometry_classification_tolerance_m,
        )?;

        let maximum_axis_divisions = maximum_triangles / 2;
        let u_sizing = least_divisions(
            frame.u_length_m,
            target_maximum_edge_length_m,
            maximum_axis_divisions,
        )?;
        let v_sizing = least_divisions(
            frame.v_length_m,
            target_maximum_edge_length_m,
            maximum_axis_divisions,
        )?;
        let u_divisions = u_sizing.divisions;
        let v_divisions = v_sizing.divisions;
        let counts = ResolvedCounts::new(u_divisions, v_divisions, maximum_triangles)?;
        let region = PlanarRegion::new(
            vec![
                [0.0, 0.0],
                [frame.u_length_m, 0.0],
                [frame.u_length_m, frame.v_length_m],
                [0.0, frame.v_length_m],
            ],
            vec![PlanarFace::new(vec![0, 1, 2, 3], Vec::new())],
            Vec::new(),
            geometry_classification_tolerance_m,
        )?;
        let topology = build_topology(
            frame.u_length_m,
            frame.v_length_m,
            u_divisions,
            v_divisions,
            u_sizing.nominal_spacing_m,
            v_sizing.nominal_spacing_m,
            counts,
        )?;
        let mesh = SimplicialMesh::new(2, topology.vertices, topology.cells, quality_gate)?;

        Ok(Self {
            source_graph_digest: graph.digest_bytes(),
            source_face: source_face.clone(),
            geometry_classification_tolerance_m,
            target_maximum_edge_length_m,
            maximum_triangles,
            origin_m: frame.origin_m,
            u_hat: frame.u_hat,
            v_hat: frame.v_hat,
            parent_outward_normal,
            u_length_m: frame.u_length_m,
            v_length_m: frame.v_length_m,
            u_divisions,
            v_divisions,
            u_maximum_realized_gap_m: u_sizing.maximum_realized_spacing_m,
            v_maximum_realized_gap_m: v_sizing.maximum_realized_spacing_m,
            region,
            mesh,
        })
    }

    /// Exact authored graph revision supplying the selected face.
    #[must_use]
    pub const fn source_graph_digest_bytes(&self) -> [u8; 32] {
        self.source_graph_digest
    }

    /// Graph-bound authored face supplying this surface realization.
    #[must_use]
    pub const fn source_face(&self) -> &CadAuthoredFaceHandle {
        &self.source_face
    }

    /// Continuous-Geometry classification tolerance in metres.
    #[must_use]
    pub const fn geometry_classification_tolerance_m(&self) -> f64 {
        self.geometry_classification_tolerance_m
    }

    /// Caller-requested maximum triangle-edge length in metres.
    #[must_use]
    pub const fn target_maximum_edge_length_m(&self) -> f64 {
        self.target_maximum_edge_length_m
    }

    /// Caller-supplied triangle work limit retained with the realization.
    #[must_use]
    pub const fn maximum_triangles(&self) -> usize {
        self.maximum_triangles
    }

    /// Selected face-cycle origin in the authored graph's 3D coordinates.
    #[must_use]
    pub const fn origin_m(&self) -> [f64; 3] {
        self.origin_m
    }

    /// Unit direction of increasing intrinsic `u`.
    #[must_use]
    pub const fn u_hat(&self) -> [f64; 3] {
        self.u_hat
    }

    /// Unit direction of increasing intrinsic `v`.
    #[must_use]
    pub const fn v_hat(&self) -> [f64; 3] {
        self.v_hat
    }

    /// Parent-relative outward unit normal of the selected authored face.
    #[must_use]
    pub const fn parent_outward_normal(&self) -> [f64; 3] {
        self.parent_outward_normal
    }

    /// Exact selected-face extent along intrinsic `u`, in metres.
    #[must_use]
    pub const fn u_length_m(&self) -> f64 {
        self.u_length_m
    }

    /// Exact selected-face extent along intrinsic `v`, in metres.
    #[must_use]
    pub const fn v_length_m(&self) -> f64 {
        self.v_length_m
    }

    /// Least accepted subdivision count along intrinsic `u`.
    #[must_use]
    pub const fn u_divisions(&self) -> usize {
        self.u_divisions
    }

    /// Least accepted subdivision count along intrinsic `v`.
    #[must_use]
    pub const fn v_divisions(&self) -> usize {
        self.v_divisions
    }

    /// Maximum realized adjacent `u`-coordinate gap, in metres.
    ///
    /// This is the measured spacing used by the acceptance predicate. It can
    /// exceed the nominal generator parameter by rounding ulps because the
    /// final coordinate is snapped to the exact authored endpoint.
    #[must_use]
    pub const fn u_spacing_m(&self) -> f64 {
        self.u_maximum_realized_gap_m
    }

    /// Maximum realized adjacent `v`-coordinate gap, in metres.
    ///
    /// This is the measured spacing used by the acceptance predicate. It can
    /// exceed the nominal generator parameter by rounding ulps because the
    /// final coordinate is snapped to the exact authored endpoint.
    #[must_use]
    pub const fn v_spacing_m(&self) -> f64 {
        self.v_maximum_realized_gap_m
    }

    /// Lift an intrinsic point to the exact retained 3D face placement.
    #[must_use]
    pub fn lift_intrinsic_point_m(&self, point_m: [f64; 2]) -> [f64; 3] {
        core::array::from_fn(|axis| {
            self.origin_m[axis] + point_m[0] * self.u_hat[axis] + point_m[1] * self.v_hat[axis]
        })
    }

    /// Canonical straight-edged intrinsic Geometry.
    #[must_use]
    pub const fn region(&self) -> &PlanarRegion {
        &self.region
    }

    /// Accepted intrinsic affine-triangle mesh.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMesh {
        &self.mesh
    }
}

#[derive(Clone, Copy)]
struct FaceFrame {
    origin_m: [f64; 3],
    u_hat: [f64; 3],
    v_hat: [f64; 3],
    u_length_m: f64,
    v_length_m: f64,
}

impl FaceFrame {
    fn from_cycle(
        cycle: [[f64; 3]; 4],
        parent_outward_normal: [f64; 3],
        tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        if cycle.iter().flatten().any(|value| !value.is_finite())
            || parent_outward_normal.iter().any(|value| !value.is_finite())
        {
            return Err(invalid(
                "authored face frame must contain finite coordinates",
            ));
        }
        let u = subtract(cycle[1], cycle[0]);
        let v = subtract(cycle[3], cycle[0]);
        let u_length_m = norm(u);
        let v_length_m = norm(v);
        if !u_length_m.is_finite()
            || u_length_m <= tolerance_m
            || !v_length_m.is_finite()
            || v_length_m <= tolerance_m
        {
            return Err(invalid(
                "authored rectangular face axes must exceed the Geometry classification tolerance",
            ));
        }
        let u_hat = scale(u, 1.0 / u_length_m);
        let v_hat = scale(v, 1.0 / v_length_m);
        let expected_opposite = add(cycle[0], add(u, v));
        if cycle[2] != expected_opposite
            || dot(u_hat, v_hat) != 0.0
            || cross(u_hat, v_hat) != parent_outward_normal
        {
            return Err(invalid(
                "authored face cycle is not a rectangle aligned with its parent-outward normal",
            ));
        }
        Ok(Self {
            origin_m: cycle[0],
            u_hat,
            v_hat,
            u_length_m,
            v_length_m,
        })
    }
}

#[derive(Clone, Copy)]
struct ResolvedCounts {
    vertices: usize,
    triangles: usize,
}

struct BuiltTopology {
    vertices: Vec<Vec<f64>>,
    cells: Vec<Vec<usize>>,
}

#[derive(Clone, Copy)]
struct AxisSizing {
    divisions: usize,
    nominal_spacing_m: f64,
    maximum_realized_spacing_m: f64,
}

impl ResolvedCounts {
    fn new(
        u_divisions: usize,
        v_divisions: usize,
        maximum_triangles: usize,
    ) -> Result<Self, Diagnostic> {
        let triangles = u_divisions
            .checked_mul(v_divisions)
            .and_then(|rectangles| rectangles.checked_mul(2))
            .ok_or_else(|| invalid("authored face triangle count overflows usize"))?;
        if triangles > maximum_triangles {
            return Err(invalid(format!(
                "authored face mesh requires {triangles} triangles, exceeding the caller limit of {maximum_triangles}",
            )));
        }
        let vertices = u_divisions
            .checked_add(1)
            .and_then(|u| v_divisions.checked_add(1).and_then(|v| u.checked_mul(v)))
            .ok_or_else(|| invalid("authored face vertex count overflows usize"))?;
        let _frontier_entries = u_divisions
            .checked_add(v_divisions)
            .and_then(|sum| sum.checked_mul(2))
            .ok_or_else(|| invalid("authored face frontier count overflows usize"))?;
        let _coordinate_scalars = vertices
            .checked_mul(2)
            .ok_or_else(|| invalid("authored face coordinate count overflows usize"))?;
        let _connectivity_indices = triangles
            .checked_mul(3)
            .ok_or_else(|| invalid("authored face connectivity count overflows usize"))?;
        Ok(Self {
            vertices,
            triangles,
        })
    }
}

fn validate_request(
    geometry_classification_tolerance_m: f64,
    target_maximum_edge_length_m: f64,
    maximum_triangles: usize,
) -> Result<(), Diagnostic> {
    if !geometry_classification_tolerance_m.is_finite()
        || geometry_classification_tolerance_m <= 0.0
    {
        return Err(invalid(
            "authored face Geometry classification tolerance must be finite and positive in metres",
        ));
    }
    if !target_maximum_edge_length_m.is_finite() || target_maximum_edge_length_m <= 0.0 {
        return Err(invalid(
            "authored face target edge length must be finite and positive in metres",
        ));
    }
    if maximum_triangles < MINIMUM_TRIANGLES {
        return Err(invalid(format!(
            "authored face work limit must admit at least {MINIMUM_TRIANGLES} triangles",
        )));
    }
    if maximum_triangles > MAXIMUM_REFERENCE_TRIANGLES {
        return Err(invalid(format!(
            "authored face caller limit {maximum_triangles} exceeds the {MAXIMUM_REFERENCE_TRIANGLES}-triangle reference-path hard limit",
        )));
    }
    Ok(())
}

fn least_divisions(length_m: f64, target_m: f64, maximum: usize) -> Result<AxisSizing, Diagnostic> {
    let nominal = least_nominal_divisions(length_m, target_m, maximum)?;
    let sizing = measure_axis_sizing(length_m, nominal)?;
    if realized_axis_satisfies_target(sizing, target_m) {
        return Ok(sizing);
    }

    let corrected = nominal
        .checked_add(1)
        .ok_or_else(|| invalid("authored face realized division correction overflows usize"))?;
    if corrected > maximum {
        return Err(invalid(format!(
            "authored face target edge length requires more than {maximum} divisions on one axis",
        )));
    }
    let sizing = measure_axis_sizing(length_m, corrected)?;
    if !realized_axis_satisfies_target(sizing, target_m) {
        return Err(invalid(
            "authored face realized-coordinate correction exceeded its bounded successor proof",
        ));
    }
    Ok(sizing)
}

fn least_nominal_divisions(
    length_m: f64,
    target_m: f64,
    maximum: usize,
) -> Result<usize, Diagnostic> {
    if maximum == 0 || !nominal_satisfies_target(length_m, target_m, maximum) {
        return Err(invalid(format!(
            "authored face target edge length requires more than {maximum} divisions on one axis",
        )));
    }

    let estimate = ((length_m / target_m) * SQRT_2).ceil();
    let mut candidate = if !estimate.is_finite() || estimate >= maximum as f64 {
        maximum
    } else if estimate <= 1.0 {
        1
    } else {
        estimate as usize
    };
    while candidate > 1 && nominal_satisfies_target(length_m, target_m, candidate - 1) {
        candidate -= 1;
    }
    while !nominal_satisfies_target(length_m, target_m, candidate) {
        candidate = candidate
            .checked_add(1)
            .ok_or_else(|| invalid("authored face division correction overflows usize"))?;
        if candidate > maximum {
            return Err(invalid(format!(
                "authored face target edge length requires more than {maximum} divisions on one axis",
            )));
        }
    }
    Ok(candidate)
}

fn nominal_satisfies_target(length_m: f64, target_m: f64, divisions: usize) -> bool {
    let spacing_m = length_m / divisions as f64;
    spacing_m.hypot(spacing_m) <= target_m
}

fn measure_axis_sizing(length_m: f64, divisions: usize) -> Result<AxisSizing, Diagnostic> {
    let nominal_spacing_m = length_m / divisions as f64;
    if !nominal_spacing_m.is_finite() || nominal_spacing_m <= 0.0 {
        return Err(invalid("authored face mesh spacing is unrepresentable"));
    }
    let mut previous = 0.0;
    let mut maximum_realized_spacing_m = 0.0_f64;
    for index in 1..=divisions {
        let coordinate = axis_coordinate(index, divisions, nominal_spacing_m, length_m)?;
        let realized_spacing_m = coordinate - previous;
        if !realized_spacing_m.is_finite() || realized_spacing_m <= 0.0 {
            return Err(invalid(
                "authored face mesh coordinate distribution is not strictly increasing",
            ));
        }
        maximum_realized_spacing_m = maximum_realized_spacing_m.max(realized_spacing_m);
        previous = coordinate;
    }
    Ok(AxisSizing {
        divisions,
        nominal_spacing_m,
        maximum_realized_spacing_m,
    })
}

fn realized_axis_satisfies_target(sizing: AxisSizing, target_m: f64) -> bool {
    sizing
        .maximum_realized_spacing_m
        .hypot(sizing.maximum_realized_spacing_m)
        <= target_m
}

fn build_topology(
    u_length_m: f64,
    v_length_m: f64,
    u_divisions: usize,
    v_divisions: usize,
    u_spacing_m: f64,
    v_spacing_m: f64,
    counts: ResolvedCounts,
) -> Result<BuiltTopology, Diagnostic> {
    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(counts.vertices)
        .map_err(|_| invalid("authored face vertex allocation failed"))?;
    for j in 0..=v_divisions {
        let v = axis_coordinate(j, v_divisions, v_spacing_m, v_length_m)?;
        for i in 0..=u_divisions {
            let u = axis_coordinate(i, u_divisions, u_spacing_m, u_length_m)?;
            vertices.push(vec![u, v]);
        }
    }

    let mut cells = Vec::new();
    cells
        .try_reserve_exact(counts.triangles)
        .map_err(|_| invalid("authored face triangle allocation failed"))?;
    let stride = u_divisions + 1;
    for j in 0..v_divisions {
        for i in 0..u_divisions {
            let a = j * stride + i;
            let b = a + 1;
            let d = a + stride;
            let c = d + 1;
            cells.push(vec![b, c, a]);
            cells.push(vec![d, a, c]);
        }
    }
    debug_assert_eq!(vertices.len(), counts.vertices);
    debug_assert_eq!(cells.len(), counts.triangles);
    Ok(BuiltTopology { vertices, cells })
}

fn axis_coordinate(
    index: usize,
    divisions: usize,
    spacing_m: f64,
    length_m: f64,
) -> Result<f64, Diagnostic> {
    let coordinate = if index == divisions {
        length_m
    } else {
        index as f64 * spacing_m
    };
    if coordinate.is_finite() {
        Ok(coordinate + 0.0)
    } else {
        Err(invalid(
            "authored face mesh coordinate is not representable in binary64 metres",
        ))
    }
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    core::array::from_fn(|axis| left[axis] + right[axis])
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    core::array::from_fn(|axis| left[axis] - right[axis])
}

fn scale(vector: [f64; 3], factor: f64) -> [f64; 3] {
    core::array::from_fn(|axis| vector[axis] * factor)
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]))
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1].mul_add(right[2], -(left[2] * right[1])),
        left[2].mul_add(right[0], -(left[0] * right[2])),
        left[0].mul_add(right[1], -(left[1] * right[0])),
    ]
}

fn norm(vector: [f64; 3]) -> f64 {
    vector[0].hypot(vector[1]).hypot(vector[2])
}
