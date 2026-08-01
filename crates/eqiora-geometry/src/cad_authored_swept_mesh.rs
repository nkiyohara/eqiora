//! Source-bound volume mesh swept through one authored rectangular body.
//!
//! The accepted surface remains the source of transverse topology and exact
//! placement. This module adds only the bounded, no-wire ownership needed to
//! retain the target body, inward layering, work policy, and accepted volume
//! mesh together; durable Geometry, Mesh, and correspondence remain owned by
//! their existing artifact envelopes.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};

use crate::{AxisAlignedBox3, CadAuthoredFaceMesh, CadAuthoredGraph};

const TETRAHEDRA_PER_TRIANGULAR_PRISM: usize = 3;
const MINIMUM_TETRAHEDRA: usize = 3;
const MAXIMUM_REFERENCE_TETRAHEDRA: usize = 1_000_000;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// One accepted authored-face mesh swept inward through its exact target body.
///
/// This value has no durable wire. It retains the complete source surface, the
/// immutable one-body target graph, source-relative geometric grading, exact
/// generated layer offsets, the caller work limit, and the accepted affine-
/// tetrahedron mesh. The existing generic Cartesian artifact boundary remains
/// responsible for accepting complete body and boundary correspondence.
#[derive(Clone, Debug, PartialEq)]
pub struct CadAuthoredSweptMesh {
    source_surface: CadAuthoredFaceMesh,
    target_graph: CadAuthoredGraph,
    inward_direction: [f64; 3],
    normal_axis: usize,
    sweep_distance_m: f64,
    layers: usize,
    growth_rate: f64,
    layer_offsets_m: Vec<f64>,
    maximum_tetrahedra: usize,
    mesh: SimplicialMesh,
}

impl CadAuthoredSweptMesh {
    /// Sweep one accepted full rectangular outer-face mesh through its body.
    ///
    /// Layer thickness grows geometrically from the source into the body, so
    /// adjacent thicknesses have the caller-supplied ratio. The sweep direction
    /// is derived as the negated parent-outward normal and cannot be supplied
    /// independently. Every triangular prism is split by the global source-
    /// vertex order, which makes the diagonal on a shared vertical quadrilateral
    /// independent of either incident triangle's stored orientation.
    ///
    /// # Errors
    /// Returns `EQ0901` before volume-topology allocation for an invalid request,
    /// a stale or foreign source, a cut or otherwise unsupported target, an
    /// incomplete source face, checked-count overflow, a work-limit violation,
    /// or non-representable layer offsets. Generated topology is then subject
    /// to the unchanged `SimplicialMesh` `EQ0803` orientation and quality gate.
    pub fn through_body(
        source_surface: &CadAuthoredFaceMesh,
        target_graph: &CadAuthoredGraph,
        layers: usize,
        growth_rate: f64,
        maximum_tetrahedra: usize,
        quality_gate: MeshQualityGate,
    ) -> Result<Self, Diagnostic> {
        validate_request(layers, growth_rate, maximum_tetrahedra)?;
        validate_target(source_surface, target_graph)?;

        let normal_axis = axis_aligned_normal_axis(source_surface.parent_outward_normal())?;
        let bounds = target_graph.output();
        validate_complete_source_face(source_surface, bounds, normal_axis)?;
        let axis_bounds = bounds.bounds_m()[normal_axis];
        let sweep_distance_m = axis_bounds.1 - axis_bounds.0;
        let inward_direction = source_surface.parent_outward_normal().map(|value| -value);
        let target_normal_coordinate_m = if inward_direction[normal_axis] < 0.0 {
            axis_bounds.0
        } else {
            axis_bounds.1
        };

        let counts = ResolvedCounts::new(source_surface, layers, maximum_tetrahedra)?;
        let layer_offsets_m = layer_offsets(sweep_distance_m, layers, growth_rate)?;
        let vertices = build_vertices(
            source_surface,
            &layer_offsets_m,
            inward_direction,
            normal_axis,
            target_normal_coordinate_m,
            counts.vertices,
        )?;
        let cells = build_cells(source_surface, layers, &vertices, counts.tetrahedra)?;
        let mesh = SimplicialMesh::new(3, vertices, cells, quality_gate)?;

        Ok(Self {
            source_surface: source_surface.clone(),
            target_graph: target_graph.clone(),
            inward_direction,
            normal_axis,
            sweep_distance_m,
            layers,
            growth_rate,
            layer_offsets_m,
            maximum_tetrahedra,
            mesh,
        })
    }

    /// Accepted source-bound surface realization.
    #[must_use]
    pub const fn source_surface(&self) -> &CadAuthoredFaceMesh {
        &self.source_surface
    }

    /// Exact one-body authored graph swept by this realization.
    #[must_use]
    pub const fn target_graph(&self) -> &CadAuthoredGraph {
        &self.target_graph
    }

    /// Exact bounds of the target graph's one admitted body.
    #[must_use]
    pub const fn target_body_bounds(&self) -> AxisAlignedBox3 {
        self.target_graph.output()
    }

    /// Unit direction from the selected source face into the target body.
    #[must_use]
    pub const fn inward_direction(&self) -> [f64; 3] {
        self.inward_direction
    }

    /// Global Cartesian axis normal to the source face.
    #[must_use]
    pub const fn normal_axis(&self) -> usize {
        self.normal_axis
    }

    /// Exact target-body width traversed by the sweep, in metres.
    #[must_use]
    pub const fn sweep_distance_m(&self) -> f64 {
        self.sweep_distance_m
    }

    /// Number of positive-thickness slabs.
    #[must_use]
    pub const fn layers(&self) -> usize {
        self.layers
    }

    /// Requested adjacent-thickness ratio from the source into the body.
    #[must_use]
    pub const fn growth_rate(&self) -> f64 {
        self.growth_rate
    }

    /// Generated source-relative layer boundaries, including both endpoints.
    #[must_use]
    pub fn layer_offsets_m(&self) -> &[f64] {
        &self.layer_offsets_m
    }

    /// Caller-supplied tetrahedron work limit retained with the realization.
    #[must_use]
    pub const fn maximum_tetrahedra(&self) -> usize {
        self.maximum_tetrahedra
    }

    /// Accepted positively oriented affine-tetrahedron volume mesh.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMesh {
        &self.mesh
    }
}

#[derive(Clone, Copy)]
struct ResolvedCounts {
    vertices: usize,
    tetrahedra: usize,
}

impl ResolvedCounts {
    fn new(
        source_surface: &CadAuthoredFaceMesh,
        layers: usize,
        maximum_tetrahedra: usize,
    ) -> Result<Self, Diagnostic> {
        let tetrahedra = source_surface
            .mesh()
            .cells()
            .len()
            .checked_mul(layers)
            .and_then(|prisms| prisms.checked_mul(TETRAHEDRA_PER_TRIANGULAR_PRISM))
            .ok_or_else(|| invalid("authored surface-sweep tetrahedron count overflows usize"))?;
        if tetrahedra > maximum_tetrahedra {
            return Err(invalid(format!(
                "authored surface sweep requires {tetrahedra} tetrahedra, exceeding the caller limit of {maximum_tetrahedra}",
            )));
        }
        let layer_vertices = layers
            .checked_add(1)
            .ok_or_else(|| invalid("authored surface-sweep layer count overflows usize"))?;
        let vertices = source_surface
            .mesh()
            .vertices()
            .len()
            .checked_mul(layer_vertices)
            .ok_or_else(|| invalid("authored surface-sweep vertex count overflows usize"))?;
        Ok(Self {
            vertices,
            tetrahedra,
        })
    }
}

fn validate_request(
    layers: usize,
    growth_rate: f64,
    maximum_tetrahedra: usize,
) -> Result<(), Diagnostic> {
    if layers == 0 {
        return Err(invalid(
            "authored surface sweep requires at least one positive-thickness layer",
        ));
    }
    if !growth_rate.is_finite() || growth_rate < 1.0 {
        return Err(invalid(
            "authored surface-sweep growth rate must be finite and at least one",
        ));
    }
    if !(MINIMUM_TETRAHEDRA..=MAXIMUM_REFERENCE_TETRAHEDRA).contains(&maximum_tetrahedra) {
        return Err(invalid(format!(
            "authored surface-sweep tetrahedron limit must lie in [{MINIMUM_TETRAHEDRA}, {MAXIMUM_REFERENCE_TETRAHEDRA}]",
        )));
    }
    Ok(())
}

fn validate_target(
    source_surface: &CadAuthoredFaceMesh,
    target_graph: &CadAuthoredGraph,
) -> Result<(), Diagnostic> {
    if source_surface.source_graph_digest_bytes() != target_graph.digest_bytes() {
        return Err(invalid(
            "authored surface sweep source belongs to a foreign or stale graph revision",
        ));
    }
    target_graph.resolve_face(source_surface.source_face())?;
    if target_graph.requested_boolean_tolerance_m().is_some()
        || target_graph.body_count() != 1
        || target_graph.face_count() != 6
        || target_graph.vertex_count() != Some(8)
    {
        return Err(invalid(
            "authored surface sweep requires the uncut one-body rectangle-extrusion graph",
        ));
    }
    Ok(())
}

fn axis_aligned_normal_axis(normal: [f64; 3]) -> Result<usize, Diagnostic> {
    let mut normal_axis = None;
    for (axis, value) in normal.into_iter().enumerate() {
        if value == 1.0 || value == -1.0 {
            if normal_axis.replace(axis).is_some() {
                return Err(invalid(
                    "authored surface sweep requires one axis-aligned source normal",
                ));
            }
        } else if value != 0.0 {
            return Err(invalid(
                "authored surface sweep requires one axis-aligned source normal",
            ));
        }
    }
    normal_axis.ok_or_else(|| {
        invalid("authored surface sweep requires one nonzero axis-aligned source normal")
    })
}

fn validate_complete_source_face(
    source_surface: &CadAuthoredFaceMesh,
    target: AxisAlignedBox3,
    normal_axis: usize,
) -> Result<(), Diagnostic> {
    let corners = [
        source_surface.lift_intrinsic_point_m([0.0, 0.0]),
        source_surface.lift_intrinsic_point_m([source_surface.u_length_m(), 0.0]),
        source_surface
            .lift_intrinsic_point_m([source_surface.u_length_m(), source_surface.v_length_m()]),
        source_surface.lift_intrinsic_point_m([0.0, source_surface.v_length_m()]),
    ];
    let bounds = target.bounds_m();
    for axis in 0..3 {
        let minimum = corners
            .iter()
            .map(|corner| corner[axis])
            .fold(f64::INFINITY, f64::min);
        let maximum = corners
            .iter()
            .map(|corner| corner[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        if axis == normal_axis {
            let expected = if source_surface.parent_outward_normal()[axis] < 0.0 {
                bounds[axis].0
            } else {
                bounds[axis].1
            };
            if minimum != expected || maximum != expected {
                return Err(invalid(
                    "authored surface sweep source is not the complete selected outer face",
                ));
            }
        } else if (minimum, maximum) != bounds[axis] {
            return Err(invalid(
                "authored surface sweep source does not cover the complete target face",
            ));
        }
    }
    Ok(())
}

fn layer_offsets(
    sweep_distance_m: f64,
    layers: usize,
    growth_rate: f64,
) -> Result<Vec<f64>, Diagnostic> {
    let mut offsets = Vec::with_capacity(
        layers
            .checked_add(1)
            .ok_or_else(|| invalid("authored surface-sweep layer count overflows usize"))?,
    );
    offsets.push(0.0);
    if layers == 1 {
        offsets.push(sweep_distance_m);
        return Ok(offsets);
    }

    if growth_rate == 1.0 {
        let spacing_m = sweep_distance_m / layers as f64;
        for layer in 1..layers {
            push_interior_offset(&mut offsets, layer as f64 * spacing_m, sweep_distance_m)?;
        }
    } else {
        // Scale the largest, final thickness to one. All preceding weights are
        // inverse powers in (0, 1], so neither their construction nor their sum
        // can overflow. An underflowed source weight is rejected as a collapsed
        // layer rather than silently omitted.
        let inverse_growth = 1.0 / growth_rate;
        let mut source_weight = 1.0;
        let mut weight_sum = 1.0;
        for _ in 1..layers {
            source_weight *= inverse_growth;
            weight_sum += source_weight;
        }
        if !source_weight.is_finite()
            || source_weight <= 0.0
            || !weight_sum.is_finite()
            || weight_sum <= 0.0
        {
            return Err(invalid(
                "authored surface-sweep grading cannot represent every positive layer",
            ));
        }
        let mut cumulative_m = 0.0;
        let mut weight = source_weight;
        for _ in 1..layers {
            cumulative_m += sweep_distance_m * (weight / weight_sum);
            push_interior_offset(&mut offsets, cumulative_m, sweep_distance_m)?;
            weight *= growth_rate;
        }
    }
    offsets.push(sweep_distance_m);
    Ok(offsets)
}

fn push_interior_offset(
    offsets: &mut Vec<f64>,
    offset_m: f64,
    sweep_distance_m: f64,
) -> Result<(), Diagnostic> {
    let previous = *offsets
        .last()
        .expect("layer offsets always retain the exact source endpoint");
    if !offset_m.is_finite() || offset_m <= previous || offset_m >= sweep_distance_m {
        return Err(invalid(
            "authored surface-sweep grading produced collapsed or non-increasing layer offsets",
        ));
    }
    offsets.push(offset_m);
    Ok(())
}

fn build_vertices(
    source_surface: &CadAuthoredFaceMesh,
    layer_offsets_m: &[f64],
    inward_direction: [f64; 3],
    normal_axis: usize,
    target_normal_coordinate_m: f64,
    expected_vertices: usize,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    let source_vertices = source_surface.mesh().vertices();
    let mut vertices = Vec::with_capacity(expected_vertices);
    for (layer, &offset_m) in layer_offsets_m.iter().enumerate() {
        for intrinsic in source_vertices {
            let mut point = source_surface.lift_intrinsic_point_m([intrinsic[0], intrinsic[1]]);
            if layer > 0 {
                point[normal_axis] += inward_direction[normal_axis] * offset_m;
            }
            if layer + 1 == layer_offsets_m.len() {
                point[normal_axis] = target_normal_coordinate_m;
            }
            if point.iter().any(|coordinate| !coordinate.is_finite()) {
                return Err(invalid(
                    "authored surface sweep produced a non-finite volume vertex",
                ));
            }
            vertices.push(point.to_vec());
        }
    }
    debug_assert_eq!(vertices.len(), expected_vertices);
    Ok(vertices)
}

fn build_cells(
    source_surface: &CadAuthoredFaceMesh,
    layers: usize,
    vertices: &[Vec<f64>],
    expected_tetrahedra: usize,
) -> Result<Vec<Vec<usize>>, Diagnostic> {
    let source_vertex_count = source_surface.mesh().vertices().len();
    let mut cells = Vec::with_capacity(expected_tetrahedra);
    for triangle in source_surface.mesh().cells() {
        let mut source = [triangle[0], triangle[1], triangle[2]];
        source.sort_unstable();
        for slab in 0..layers {
            let bottom = source.map(|vertex| slab * source_vertex_count + vertex);
            let top = source.map(|vertex| (slab + 1) * source_vertex_count + vertex);
            for mut cell in [
                [bottom[0], bottom[1], bottom[2], top[2]],
                [bottom[0], bottom[1], top[1], top[2]],
                [bottom[0], top[0], top[1], top[2]],
            ] {
                orient_cell(vertices, &mut cell)?;
                cells.push(cell.to_vec());
            }
        }
    }
    debug_assert_eq!(cells.len(), expected_tetrahedra);
    Ok(cells)
}

fn orient_cell(vertices: &[Vec<f64>], cell: &mut [usize; 4]) -> Result<(), Diagnostic> {
    let origin = &vertices[cell[0]];
    let column = |vertex: usize, axis: usize| vertices[cell[vertex]][axis] - origin[axis];
    let determinant = column(1, 0) * (column(2, 1) * column(3, 2) - column(3, 1) * column(2, 2))
        - column(2, 0) * (column(1, 1) * column(3, 2) - column(3, 1) * column(1, 2))
        + column(3, 0) * (column(1, 1) * column(2, 2) - column(2, 1) * column(1, 2));
    if !determinant.is_finite() || determinant == 0.0 {
        return Err(invalid(
            "authored surface sweep produced a degenerate or non-finite tetrahedron",
        ));
    }
    if determinant < 0.0 {
        cell.swap(1, 2);
    }
    Ok(())
}
