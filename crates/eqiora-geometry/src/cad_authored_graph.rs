//! One closed provider-neutral authored CAD operation graph.
//!
//! The first schema owns exactly one dependency chain:
//!
//! ```text
//! XY sketch plane -> constrained rectangle -> closed face -> positive-z extrusion
//! ```
//!
//! It is deliberately not a general feature enum or B-rep.  Its canonical
//! identity contains authored inputs and their dependencies, while the exact
//! analytic solid and provenance faces are derived values.  No provider object,
//! face number, mesh policy, or presentation metadata crosses this boundary.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::cad_authored_selection::{CadAuthoredFaceHandleV1, CadAuthoredFaceSelectionV1};
use crate::canonical::{CANONICAL_ENCODING, WireLengthUnit, digest_with_schema};
use crate::{AxisAlignedBox3, CadRepairDispositionV1, ConstrainedRectangleV1};

const GRAPH_SCHEMA: &str = "eqiora.cad-authored-operation-graph-envelope/v1";
const MAX_GRAPH_BYTES: usize = 4_096;
const SKETCH_PLANE_ID: &str = "sketch-plane";
const PROFILE_ID: &str = "rectangle-profile";
const FACE_ID: &str = "profile-face";
const EXTRUSION_ID: &str = "positive-z-extrusion";

const VERTEX_COUNT: usize = 8;
const EDGE_COUNT: usize = 12;
const FACE_COUNT: usize = 6;
const SHELL_COUNT: usize = 1;
const BODY_COUNT: usize = 1;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Exact planar face derived from one authored-provenance selection.
///
/// Vertices form an outward-oriented cycle.  The remaining quantities are
/// derived from that cycle rather than supplied by a CAD provider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadAuthoredPlanarFaceV1 {
    selection: CadAuthoredFaceSelectionV1,
    vertices_m: [[f64; 3]; 4],
    centroid_m: [f64; 3],
    area_m2: f64,
    outward_normal: [f64; 3],
}

impl CadAuthoredPlanarFaceV1 {
    /// Authored provenance that owns this face.
    #[must_use]
    pub const fn selection(self) -> CadAuthoredFaceSelectionV1 {
        self.selection
    }

    /// Four coherent-SI vertices in outward-oriented cyclic order.
    #[must_use]
    pub const fn vertices_m(self) -> [[f64; 3]; 4] {
        self.vertices_m
    }

    /// Exact face centroid in coherent-SI metres.
    #[must_use]
    pub const fn centroid_m(self) -> [f64; 3] {
        self.centroid_m
    }

    /// Exact planar area in square metres.
    #[must_use]
    pub const fn area_m2(self) -> f64 {
        self.area_m2
    }

    /// Exact unit normal pointing out of the authored solid.
    #[must_use]
    pub const fn outward_normal(self) -> [f64; 3] {
        self.outward_normal
    }
}

/// Immutable canonical meaning of the first authored CAD operation graph.
///
/// Construction closes the rectangle by type, closes exactly one face from
/// that loop, and extrudes only in positive z.  A later graph vocabulary uses
/// another schema domain; it cannot reinterpret this value or its handles.
#[derive(Clone, Debug, PartialEq)]
pub struct CadAuthoredGraphV1 {
    sketch: ConstrainedRectangleV1,
    extrusion_depth_m: f64,
    requested_modeling_tolerance_m: f64,
    output: AxisAlignedBox3,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CadAuthoredGraphV1 {
    /// Close the fixed sketch/profile/face/extrusion dependency chain.
    ///
    /// The requested tolerance is recorded in identity only.  It is never
    /// applied to coordinates, topology, face classification, or repair.
    ///
    /// # Errors
    /// Returns `EQ0901` for a non-positive or non-finite depth/tolerance, for
    /// non-finite derived extents, or for an unexpected serialization failure.
    pub fn new(
        sketch: ConstrainedRectangleV1,
        extrusion_depth_m: f64,
        requested_modeling_tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        if !extrusion_depth_m.is_finite() || extrusion_depth_m <= 0.0 {
            return Err(invalid(
                "authored CAD extrusion depth must be finite and positive in metres",
            ));
        }
        if !requested_modeling_tolerance_m.is_finite() || requested_modeling_tolerance_m <= 0.0 {
            return Err(invalid(
                "authored CAD modeling tolerance must be finite and positive in metres",
            ));
        }

        let extrusion_depth_m = canonical_zero(extrusion_depth_m);
        let requested_modeling_tolerance_m = canonical_zero(requested_modeling_tolerance_m);
        let output = sketch.extruded_box(extrusion_depth_m)?;
        if output
            .bounds_m()
            .iter()
            .any(|&(lower, upper)| !(upper - lower).is_finite())
        {
            return Err(invalid(
                "authored CAD derived spans must remain finite in metres",
            ));
        }

        let wire = WireCadAuthoredGraphV1::from_parts(
            sketch,
            extrusion_depth_m,
            requested_modeling_tolerance_m,
        );
        let bytes = serde_json::to_vec(&wire)
            .map_err(|error| invalid(format!("cannot serialize authored CAD graph: {error}")))?;
        Ok(Self {
            sketch,
            extrusion_depth_m,
            requested_modeling_tolerance_m,
            output,
            digest: digest_with_schema(GRAPH_SCHEMA, &bytes),
            bytes,
        })
    }

    /// Decode one bounded graph through the closed schema vocabulary.
    ///
    /// Object-member order and equivalent numeric spellings are nonsemantic;
    /// reconstruction always emits the one canonical byte form.  Duplicate or
    /// unknown members and unsupported node dependencies reject.
    ///
    /// # Errors
    /// Returns `EQ0901` for excess bytes, malformed or unknown wire data, an
    /// invalid dependency chain, or invalid authored inputs.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_GRAPH_BYTES {
            return Err(invalid(format!(
                "authored CAD graph has {} bytes, exceeding the {MAX_GRAPH_BYTES} byte decoder limit",
                bytes.len(),
            )));
        }
        let wire: WireCadAuthoredGraphV1 = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid authored CAD graph JSON: {error}")))?;
        wire.check_contract()?;
        let sketch = ConstrainedRectangleV1::new(
            (wire.profile.x_bounds_m[0], wire.profile.x_bounds_m[1]),
            (wire.profile.y_bounds_m[0], wire.profile.y_bounds_m[1]),
            wire.sketch_plane.z_m,
        )?;
        Self::new(
            sketch,
            wire.extrusion.depth_m,
            wire.requested_modeling_tolerance_m,
        )
    }

    /// Fully constrained rectangle owned by the graph.
    #[must_use]
    pub const fn sketch(&self) -> ConstrainedRectangleV1 {
        self.sketch
    }

    /// Positive-z extrusion depth in metres.
    #[must_use]
    pub const fn extrusion_depth_m(&self) -> f64 {
        self.extrusion_depth_m
    }

    /// Requested graph-global modeling tolerance in metres.
    #[must_use]
    pub const fn requested_modeling_tolerance_m(&self) -> f64 {
        self.requested_modeling_tolerance_m
    }

    /// Exact analytic solid bounds; tolerance is not applied.
    #[must_use]
    pub const fn output(&self) -> AxisAlignedBox3 {
        self.output
    }

    /// The only repair disposition admitted by schema v1.
    #[must_use]
    pub const fn repair_disposition(&self) -> CadRepairDispositionV1 {
        CadRepairDispositionV1::None
    }

    /// Exact compact canonical JSON bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Complete domain-separated authored-graph identity.
    #[must_use]
    pub const fn digest_bytes(&self) -> [u8; 32] {
        self.digest
    }

    /// Vertices in canonical lower-then-upper order `A,B,C,D,A',B',C',D'`.
    #[must_use]
    pub fn vertices_m(&self) -> [[f64; 3]; VERTEX_COUNT] {
        let [(x0, x1), (y0, y1), (z0, z1)] = self.output.bounds_m();
        [
            [x0, y0, z0],
            [x1, y0, z0],
            [x1, y1, z0],
            [x0, y1, z0],
            [x0, y0, z1],
            [x1, y0, z1],
            [x1, y1, z1],
            [x0, y1, z1],
        ]
    }

    /// Number of exact vertices.
    #[must_use]
    pub const fn vertex_count(&self) -> usize {
        VERTEX_COUNT
    }

    /// Number of exact edges.
    #[must_use]
    pub const fn edge_count(&self) -> usize {
        EDGE_COUNT
    }

    /// Number of exact provenance faces.
    #[must_use]
    pub const fn face_count(&self) -> usize {
        FACE_COUNT
    }

    /// Number of connected closed shells.
    #[must_use]
    pub const fn closed_shell_count(&self) -> usize {
        SHELL_COUNT
    }

    /// Number of solid bodies.
    #[must_use]
    pub const fn body_count(&self) -> usize {
        BODY_COUNT
    }

    /// Exact analytic body volume in cubic metres.
    #[must_use]
    pub fn volume_m3(&self) -> f64 {
        let [(x0, x1), (y0, y1), (z0, z1)] = self.output.bounds_m();
        (x1 - x0) * (y1 - y0) * (z1 - z0)
    }

    /// Exact analytic total surface area in square metres.
    #[must_use]
    pub fn surface_area_m2(&self) -> f64 {
        CadAuthoredFaceSelectionV1::ALL
            .into_iter()
            .map(|selection| self.face_for(selection).area_m2())
            .sum()
    }

    /// Create one durable handle bound to this exact graph identity.
    ///
    /// # Errors
    /// Returns `EQ0901` only if canonical handle serialization unexpectedly
    /// fails.
    pub fn face_handle(
        &self,
        selection: CadAuthoredFaceSelectionV1,
    ) -> Result<CadAuthoredFaceHandleV1, Diagnostic> {
        CadAuthoredFaceHandleV1::bind(self.digest, selection)
    }

    /// Resolve one handle only after its exact graph identity matches.
    ///
    /// # Errors
    /// Returns `EQ0901` before lookup when the handle belongs to any other
    /// graph, including an analytically equal graph with different authored
    /// tolerance.
    pub fn resolve_face(
        &self,
        handle: &CadAuthoredFaceHandleV1,
    ) -> Result<CadAuthoredPlanarFaceV1, Diagnostic> {
        if handle.graph_digest_bytes() != self.digest {
            return Err(invalid(
                "CAD face handle belongs to a foreign authored graph identity",
            ));
        }
        Ok(self.face_for(handle.selection()))
    }

    fn face_for(&self, selection: CadAuthoredFaceSelectionV1) -> CadAuthoredPlanarFaceV1 {
        let [a, b, c, d, upper_a, upper_b, upper_c, upper_d] = self.vertices_m();
        let vertices = match selection {
            CadAuthoredFaceSelectionV1::StartCap => [a, d, c, b],
            CadAuthoredFaceSelectionV1::EndCap => [upper_a, upper_b, upper_c, upper_d],
            CadAuthoredFaceSelectionV1::ProfileXLower => [a, upper_a, upper_d, d],
            CadAuthoredFaceSelectionV1::ProfileXUpper => [b, c, upper_c, upper_b],
            CadAuthoredFaceSelectionV1::ProfileYLower => [a, b, upper_b, upper_a],
            CadAuthoredFaceSelectionV1::ProfileYUpper => [d, upper_d, upper_c, c],
        };
        face_from_cycle(selection, vertices)
    }
}

fn face_from_cycle(
    selection: CadAuthoredFaceSelectionV1,
    vertices_m: [[f64; 3]; 4],
) -> CadAuthoredPlanarFaceV1 {
    let first = subtract(vertices_m[1], vertices_m[0]);
    let second = subtract(vertices_m[3], vertices_m[0]);
    let area_vector = cross(first, second);
    let area_m2 = dot(area_vector, area_vector).sqrt();
    let outward_normal = area_vector.map(|component| canonical_zero(component / area_m2));
    let centroid_m = core::array::from_fn(|axis| {
        canonical_zero(vertices_m.iter().map(|vertex| vertex[axis]).sum::<f64>() / 4.0)
    });
    CadAuthoredPlanarFaceV1 {
        selection,
        vertices_m,
        centroid_m,
        area_m2,
        outward_normal,
    }
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    core::array::from_fn(|axis| left[axis] - right[axis])
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left.into_iter().zip(right).map(|(a, b)| a * b).sum()
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCadAuthoredGraphV1 {
    schema: String,
    encoding: String,
    length_unit: WireLengthUnit,
    requested_modeling_tolerance_m: f64,
    sketch_plane: WireSketchPlane,
    profile: WireRectangleProfile,
    face: WireClosedFace,
    extrusion: WirePositiveZExtrusion,
    selections: Vec<CadAuthoredFaceSelectionV1>,
}

impl WireCadAuthoredGraphV1 {
    fn from_parts(
        sketch: ConstrainedRectangleV1,
        extrusion_depth_m: f64,
        requested_modeling_tolerance_m: f64,
    ) -> Self {
        Self {
            schema: GRAPH_SCHEMA.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            length_unit: WireLengthUnit::Metre,
            requested_modeling_tolerance_m,
            sketch_plane: WireSketchPlane {
                id: SKETCH_PLANE_ID.to_owned(),
                kind: WireSketchPlaneKind::Xy,
                z_m: sketch.plane_z_m(),
            },
            profile: WireRectangleProfile {
                id: PROFILE_ID.to_owned(),
                kind: WireProfileKind::AxisAlignedRectangle,
                sketch_plane: SKETCH_PLANE_ID.to_owned(),
                constraint: WireConstraint::ClosedByConstruction,
                x_bounds_m: [sketch.x_bounds_m().0, sketch.x_bounds_m().1],
                y_bounds_m: [sketch.y_bounds_m().0, sketch.y_bounds_m().1],
            },
            face: WireClosedFace {
                id: FACE_ID.to_owned(),
                kind: WireFaceKind::OneClosedLoopFace,
                profile: PROFILE_ID.to_owned(),
                region_count: 1,
            },
            extrusion: WirePositiveZExtrusion {
                id: EXTRUSION_ID.to_owned(),
                kind: WireExtrusionKind::PositiveZ,
                face: FACE_ID.to_owned(),
                depth_m: extrusion_depth_m,
                repair: WireRepairDisposition::None,
            },
            selections: CadAuthoredFaceSelectionV1::ALL.to_vec(),
        }
    }

    fn check_contract(&self) -> Result<(), Diagnostic> {
        if self.schema != GRAPH_SCHEMA
            || self.encoding != CANONICAL_ENCODING
            || self.length_unit != WireLengthUnit::Metre
            || self.sketch_plane.id != SKETCH_PLANE_ID
            || self.sketch_plane.kind != WireSketchPlaneKind::Xy
            || self.profile.id != PROFILE_ID
            || self.profile.kind != WireProfileKind::AxisAlignedRectangle
            || self.profile.sketch_plane != SKETCH_PLANE_ID
            || self.profile.constraint != WireConstraint::ClosedByConstruction
            || self.face.id != FACE_ID
            || self.face.kind != WireFaceKind::OneClosedLoopFace
            || self.face.profile != PROFILE_ID
            || self.face.region_count != 1
            || self.extrusion.id != EXTRUSION_ID
            || self.extrusion.kind != WireExtrusionKind::PositiveZ
            || self.extrusion.face != FACE_ID
            || self.extrusion.repair != WireRepairDisposition::None
            || self.selections.as_slice() != CadAuthoredFaceSelectionV1::ALL
        {
            return Err(invalid(
                "unsupported authored CAD schema, dependency chain, selection inventory, unit, or repair disposition",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSketchPlane {
    id: String,
    kind: WireSketchPlaneKind,
    z_m: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSketchPlaneKind {
    Xy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRectangleProfile {
    id: String,
    kind: WireProfileKind,
    sketch_plane: String,
    constraint: WireConstraint,
    x_bounds_m: [f64; 2],
    y_bounds_m: [f64; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireProfileKind {
    AxisAlignedRectangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireConstraint {
    ClosedByConstruction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireClosedFace {
    id: String,
    kind: WireFaceKind,
    profile: String,
    region_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireFaceKind {
    OneClosedLoopFace,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePositiveZExtrusion {
    id: String,
    kind: WireExtrusionKind,
    face: String,
    depth_m: f64,
    repair: WireRepairDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireExtrusionKind {
    PositiveZ,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireRepairDisposition {
    None,
}
