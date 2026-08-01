//! Immutable provider-neutral authored CAD meaning.
//!
//! One opaque Rust owner replays two closed persisted operation histories:
//! the frozen rectangle-extrusion v1 wire and the bounded circular-through-cut
//! v2 wire.  The private sum is not a public feature enum or general B-rep.

use std::f64::consts::PI;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::cad_authored_build::CadAuthoredBuild;
use crate::cad_authored_cut::{CircularThroughCut, GRAPH_SCHEMA_V2, decode_v2, encode_v2};
use crate::cad_authored_selection::{
    CadAuthoredFaceHandle, CadAuthoredFaceSelection, WireFaceSelectionV1,
};
use crate::canonical::{CANONICAL_ENCODING, WireLengthUnit, digest_with_schema};
use crate::{AxisAlignedBox3, CadRepairDispositionV1, ConstrainedRectangleV1};

const GRAPH_SCHEMA_V1: &str = "eqiora.cad-authored-operation-graph-envelope/v1";
const MAX_GRAPH_BYTES: usize = 4_096;
const SKETCH_PLANE_ID: &str = "sketch-plane";
const PROFILE_ID: &str = "rectangle-profile";
const FACE_ID: &str = "profile-face";
const EXTRUSION_ID: &str = "positive-z-extrusion";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GraphCore {
    sketch: ConstrainedRectangleV1,
    extrusion_depth_m: f64,
    requested_modeling_tolerance_m: f64,
    bounds: AxisAlignedBox3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum GraphKind {
    RectangleExtrusion,
    CircularThroughCut(CircularThroughCut),
}

/// Common immutable owner of the admitted authored-CAD operation histories.
///
/// Persisted v1 and v2 schemas remain closed private variants.  Existing v1
/// bytes and digests are reproduced exactly while callers use one feature-
/// neutral Rust surface before Python and Studio projection.
#[derive(Clone, Debug, PartialEq)]
pub struct CadAuthoredGraph {
    core: GraphCore,
    kind: GraphKind,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl CadAuthoredGraph {
    /// Construct the frozen rectangle → face → positive-z extrusion history.
    ///
    /// The requested modeling tolerance is identity-only.  It is never used
    /// as a coordinate offset, classification tolerance, or repair policy.
    ///
    /// # Errors
    /// Returns `EQ0901` for a non-positive/non-finite depth or tolerance, a
    /// non-finite derived extent, or unexpected canonical serialization.
    pub fn new(
        sketch: ConstrainedRectangleV1,
        extrusion_depth_m: f64,
        requested_modeling_tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        let core = validate_core(sketch, extrusion_depth_m, requested_modeling_tolerance_m)?;
        let wire = WireCadAuthoredGraphV1::from_core(core);
        let bytes = serde_json::to_vec(&wire)
            .map_err(|error| invalid(format!("cannot serialize authored CAD graph: {error}")))?;
        Ok(Self::from_encoded(
            core,
            GraphKind::RectangleExtrusion,
            GRAPH_SCHEMA_V1,
            bytes,
        ))
    }

    /// Append the one admitted circular through-all difference operation.
    ///
    /// The cut starts on the predecessor end cap and proceeds through all in
    /// negative z.  Signed inward side clearance must exceed the requested
    /// Boolean tolerance; no tolerance substitution or healing is available.
    ///
    /// # Errors
    /// Returns `EQ0901` unless the predecessor is the closed v1 history and
    /// the finite positive circle lies strictly inside every rectangle side by
    /// more than the requested Boolean tolerance.
    pub fn circular_through_cut(
        &self,
        center_m: [f64; 2],
        radius_m: f64,
        requested_boolean_tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        if !matches!(self.kind, GraphKind::RectangleExtrusion) {
            return Err(invalid(
                "authored CAD v2 admits exactly one cut after the rectangle extrusion",
            ));
        }
        let cut = CircularThroughCut::new(
            self.core.sketch,
            center_m,
            radius_m,
            requested_boolean_tolerance_m,
        )?;
        let bytes = encode_v2(
            self.core.sketch,
            self.core.extrusion_depth_m,
            self.core.requested_modeling_tolerance_m,
            cut,
        )?;
        Ok(Self::from_encoded(
            self.core,
            GraphKind::CircularThroughCut(cut),
            GRAPH_SCHEMA_V2,
            bytes,
        ))
    }

    /// Decode either closed schema and reconstruct its one canonical byte form.
    ///
    /// Object-member order and equivalent numeric spellings are nonsemantic;
    /// duplicate/unknown members and unsupported dependencies reject.
    ///
    /// # Errors
    /// Returns `EQ0901` for excess bytes or a malformed/unsupported graph.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_GRAPH_BYTES {
            return Err(invalid(format!(
                "authored CAD graph has {} bytes, exceeding the {MAX_GRAPH_BYTES} byte decoder limit",
                bytes.len()
            )));
        }
        if let Ok(wire) = serde_json::from_slice::<WireCadAuthoredGraphV1>(bytes) {
            wire.check_contract()?;
            let sketch = ConstrainedRectangleV1::new(
                (wire.profile.x_bounds_m[0], wire.profile.x_bounds_m[1]),
                (wire.profile.y_bounds_m[0], wire.profile.y_bounds_m[1]),
                wire.sketch_plane.z_m,
            )?;
            return Self::new(
                sketch,
                wire.extrusion.depth_m,
                wire.requested_modeling_tolerance_m,
            );
        }
        if let Ok((sketch, depth, modeling_tolerance, cut)) = decode_v2(bytes) {
            let base = Self::new(sketch, depth, modeling_tolerance)?;
            return base.circular_through_cut(
                cut.center_m(),
                cut.radius_m(),
                cut.requested_tolerance_m(),
            );
        }
        Err(invalid("unsupported or malformed authored CAD graph wire"))
    }

    fn from_encoded(core: GraphCore, kind: GraphKind, schema: &str, bytes: Vec<u8>) -> Self {
        Self {
            core,
            kind,
            digest: digest_with_schema(schema, &bytes),
            bytes,
        }
    }

    /// Fully constrained rectangle owned by the graph.
    #[must_use]
    pub const fn sketch(&self) -> ConstrainedRectangleV1 {
        self.core.sketch
    }

    /// Positive-z extrusion depth in metres.
    #[must_use]
    pub const fn extrusion_depth_m(&self) -> f64 {
        self.core.extrusion_depth_m
    }

    /// Identity-bearing base modeling tolerance in metres.
    #[must_use]
    pub const fn requested_modeling_tolerance_m(&self) -> f64 {
        self.core.requested_modeling_tolerance_m
    }

    /// Requested Boolean tolerance, absent from the rectangle-only history.
    #[must_use]
    pub const fn requested_boolean_tolerance_m(&self) -> Option<f64> {
        match self.kind {
            GraphKind::RectangleExtrusion => None,
            GraphKind::CircularThroughCut(cut) => Some(cut.requested_tolerance_m()),
        }
    }

    /// Circular cut centre, absent from the rectangle-only history.
    #[must_use]
    pub const fn cut_center_m(&self) -> Option<[f64; 2]> {
        match self.kind {
            GraphKind::RectangleExtrusion => None,
            GraphKind::CircularThroughCut(cut) => Some(cut.center_m()),
        }
    }

    /// Circular cut radius, absent from the rectangle-only history.
    #[must_use]
    pub const fn cut_radius_m(&self) -> Option<f64> {
        match self.kind {
            GraphKind::RectangleExtrusion => None,
            GraphKind::CircularThroughCut(cut) => Some(cut.radius_m()),
        }
    }

    /// Exact outer analytic bounds; the through-cut does not change them.
    #[must_use]
    pub const fn output(&self) -> AxisAlignedBox3 {
        self.core.bounds
    }

    /// Neither admitted history performs healing or topology repair.
    #[must_use]
    pub const fn repair_disposition(&self) -> CadRepairDispositionV1 {
        CadRepairDispositionV1::None
    }

    /// Exact compact canonical JSON bytes for the graph's closed wire variant.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Complete domain-separated authored-graph identity.
    #[must_use]
    pub const fn digest_bytes(&self) -> [u8; 32] {
        self.digest
    }

    /// Outer rectangle-extrusion corners in lower-then-upper order.
    #[must_use]
    pub fn vertices_m(&self) -> [[f64; 3]; 8] {
        let [(x0, x1), (y0, y1), (z0, z1)] = self.core.bounds.bounds_m();
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

    /// Exact vertex count only where the analytic representation is closed.
    #[must_use]
    pub const fn vertex_count(&self) -> Option<usize> {
        if matches!(self.kind, GraphKind::RectangleExtrusion) {
            Some(8)
        } else {
            None
        }
    }

    /// Exact edge count only where the analytic representation is closed.
    #[must_use]
    pub const fn edge_count(&self) -> Option<usize> {
        if matches!(self.kind, GraphKind::RectangleExtrusion) {
            Some(12)
        } else {
            None
        }
    }

    /// Exact face count.
    #[must_use]
    pub const fn face_count(&self) -> usize {
        if matches!(self.kind, GraphKind::RectangleExtrusion) {
            6
        } else {
            7
        }
    }

    /// Number of connected closed shells.
    #[must_use]
    pub const fn closed_shell_count(&self) -> usize {
        1
    }

    /// Number of solid bodies.
    #[must_use]
    pub const fn body_count(&self) -> usize {
        1
    }

    /// Exact body genus.
    #[must_use]
    pub const fn genus(&self) -> usize {
        if matches!(self.kind, GraphKind::RectangleExtrusion) {
            0
        } else {
            1
        }
    }

    /// Exact analytic body volume in cubic metres.
    #[must_use]
    pub fn volume_m3(&self) -> f64 {
        let [(x0, x1), (y0, y1), (z0, z1)] = self.core.bounds.bounds_m();
        let outer = (x1 - x0) * (y1 - y0) * (z1 - z0);
        match self.kind {
            GraphKind::RectangleExtrusion => outer,
            GraphKind::CircularThroughCut(cut) => {
                outer - PI * cut.radius_m().powi(2) * self.core.extrusion_depth_m
            }
        }
    }

    /// Exact analytic total surface area in square metres.
    #[must_use]
    pub fn surface_area_m2(&self) -> f64 {
        self.selection_inventory()
            .iter()
            .copied()
            .map(|selection| self.face_area_for(selection))
            .sum()
    }

    /// Every admitted face provenance in canonical order.
    #[must_use]
    pub fn selection_inventory(&self) -> &'static [CadAuthoredFaceSelection] {
        match self.kind {
            GraphKind::RectangleExtrusion => &CadAuthoredFaceSelection::V1_ALL,
            GraphKind::CircularThroughCut(_) => &CadAuthoredFaceSelection::V2_ALL,
        }
    }

    /// Bind one admitted selection to this exact graph identity.
    ///
    /// # Errors
    /// Returns `EQ0901` for a selection outside this graph's closed inventory
    /// or an unexpected canonical handle serialization failure.
    pub fn face_handle(
        &self,
        selection: CadAuthoredFaceSelection,
    ) -> Result<CadAuthoredFaceHandle, Diagnostic> {
        if !self.selection_inventory().contains(&selection) {
            return Err(invalid(
                "face selection is not admitted by this authored graph",
            ));
        }
        match self.kind {
            GraphKind::RectangleExtrusion => CadAuthoredFaceHandle::bind_v1(self.digest, selection),
            GraphKind::CircularThroughCut(_) => {
                CadAuthoredFaceHandle::bind_v2(self.digest, selection)
            }
        }
    }

    /// Validate and resolve one graph-bound handle to authored provenance.
    ///
    /// # Errors
    /// Returns `EQ0901` before lookup for any foreign/stale graph digest or
    /// handle schema, then rejects a selection outside this graph inventory.
    pub fn resolve_face(
        &self,
        handle: &CadAuthoredFaceHandle,
    ) -> Result<CadAuthoredFaceSelection, Diagnostic> {
        let expected_v1 = matches!(self.kind, GraphKind::RectangleExtrusion);
        if handle.graph_digest_bytes() != self.digest || handle.is_v1() != expected_v1 {
            return Err(invalid(
                "CAD face handle belongs to a foreign authored graph identity or wire variant",
            ));
        }
        let selection = handle.selection();
        if !self.selection_inventory().contains(&selection) {
            return Err(invalid(
                "CAD face handle selection is absent from this graph",
            ));
        }
        Ok(selection)
    }

    /// Exact face area after validating the graph-bound handle.
    pub fn face_area_m2(&self, handle: &CadAuthoredFaceHandle) -> Result<f64, Diagnostic> {
        Ok(self.face_area_for(self.resolve_face(handle)?))
    }

    /// Exact number of analytic boundary loops on the selected face.
    pub fn face_boundary_loop_count(
        &self,
        handle: &CadAuthoredFaceHandle,
    ) -> Result<usize, Diagnostic> {
        let selection = self.resolve_face(handle)?;
        Ok(match self.kind {
            GraphKind::CircularThroughCut(_)
                if selection == CadAuthoredFaceSelection::start_cap()
                    || selection == CadAuthoredFaceSelection::end_cap()
                    || selection == CadAuthoredFaceSelection::cut_wall() =>
            {
                2
            }
            _ => 1,
        })
    }

    /// Four outward-oriented vertices for a rectangular face, when applicable.
    pub fn rectangular_face_vertices_m(
        &self,
        handle: &CadAuthoredFaceHandle,
    ) -> Result<Option<[[f64; 3]; 4]>, Diagnostic> {
        let selection = self.resolve_face(handle)?;
        Ok(self.rectangular_face_cycle(selection))
    }

    /// Centroid of a rectangular selected face, when applicable.
    pub fn rectangular_face_centroid_m(
        &self,
        handle: &CadAuthoredFaceHandle,
    ) -> Result<Option<[f64; 3]>, Diagnostic> {
        Ok(self.rectangular_face_vertices_m(handle)?.map(centroid))
    }

    /// Constant outward unit normal for a planar selected face.
    pub fn planar_face_outward_normal(
        &self,
        handle: &CadAuthoredFaceHandle,
    ) -> Result<Option<[f64; 3]>, Diagnostic> {
        let selection = self.resolve_face(handle)?;
        Ok(if selection == CadAuthoredFaceSelection::cut_wall() {
            None
        } else {
            Some(planar_normal(selection))
        })
    }

    /// Execute the bounded built-in analytic profile and close its receipt.
    ///
    /// # Errors
    /// Returns `EQ0901` only if graph-bound lineage handles cannot be formed.
    pub fn build_analytic(&self) -> Result<CadAuthoredBuild, Diagnostic> {
        CadAuthoredBuild::from_graph(self)
    }

    fn face_area_for(&self, selection: CadAuthoredFaceSelection) -> f64 {
        let [(x0, x1), (y0, y1), (z0, z1)] = self.core.bounds.bounds_m();
        let width = x1 - x0;
        let height = y1 - y0;
        let depth = z1 - z0;
        if selection == CadAuthoredFaceSelection::start_cap()
            || selection == CadAuthoredFaceSelection::end_cap()
        {
            let outer = width * height;
            return match self.kind {
                GraphKind::RectangleExtrusion => outer,
                GraphKind::CircularThroughCut(cut) => outer - PI * cut.radius_m().powi(2),
            };
        }
        if selection == CadAuthoredFaceSelection::profile_x_lower()
            || selection == CadAuthoredFaceSelection::profile_x_upper()
        {
            return height * depth;
        }
        if selection == CadAuthoredFaceSelection::profile_y_lower()
            || selection == CadAuthoredFaceSelection::profile_y_upper()
        {
            return width * depth;
        }
        match self.kind {
            GraphKind::CircularThroughCut(cut) => 2.0 * PI * cut.radius_m() * depth,
            GraphKind::RectangleExtrusion => unreachable!("inventory excludes cut wall"),
        }
    }

    fn rectangular_face_cycle(&self, selection: CadAuthoredFaceSelection) -> Option<[[f64; 3]; 4]> {
        if matches!(self.kind, GraphKind::CircularThroughCut(_))
            && (selection == CadAuthoredFaceSelection::start_cap()
                || selection == CadAuthoredFaceSelection::end_cap()
                || selection == CadAuthoredFaceSelection::cut_wall())
        {
            return None;
        }
        let [a, b, c, d, upper_a, upper_b, upper_c, upper_d] = self.vertices_m();
        Some(if selection == CadAuthoredFaceSelection::start_cap() {
            [a, d, c, b]
        } else if selection == CadAuthoredFaceSelection::end_cap() {
            [upper_a, upper_b, upper_c, upper_d]
        } else if selection == CadAuthoredFaceSelection::profile_x_lower() {
            [a, upper_a, upper_d, d]
        } else if selection == CadAuthoredFaceSelection::profile_x_upper() {
            [b, c, upper_c, upper_b]
        } else if selection == CadAuthoredFaceSelection::profile_y_lower() {
            [a, b, upper_b, upper_a]
        } else {
            [d, upper_d, upper_c, c]
        })
    }

    pub(crate) const fn is_cut(&self) -> bool {
        matches!(self.kind, GraphKind::CircularThroughCut(_))
    }
}

fn validate_core(
    sketch: ConstrainedRectangleV1,
    extrusion_depth_m: f64,
    requested_modeling_tolerance_m: f64,
) -> Result<GraphCore, Diagnostic> {
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
    let bounds = sketch.extruded_box(extrusion_depth_m)?;
    if bounds
        .bounds_m()
        .iter()
        .any(|&(lower, upper)| !(upper - lower).is_finite())
    {
        return Err(invalid(
            "authored CAD derived spans must remain finite in metres",
        ));
    }
    Ok(GraphCore {
        sketch,
        extrusion_depth_m,
        requested_modeling_tolerance_m,
        bounds,
    })
}

fn centroid(vertices: [[f64; 3]; 4]) -> [f64; 3] {
    core::array::from_fn(|axis| {
        canonical_zero(vertices.iter().map(|vertex| vertex[axis]).sum::<f64>() / 4.0)
    })
}

fn planar_normal(selection: CadAuthoredFaceSelection) -> [f64; 3] {
    if selection == CadAuthoredFaceSelection::start_cap() {
        [0.0, 0.0, -1.0]
    } else if selection == CadAuthoredFaceSelection::end_cap() {
        [0.0, 0.0, 1.0]
    } else if selection == CadAuthoredFaceSelection::profile_x_lower() {
        [-1.0, 0.0, 0.0]
    } else if selection == CadAuthoredFaceSelection::profile_x_upper() {
        [1.0, 0.0, 0.0]
    } else if selection == CadAuthoredFaceSelection::profile_y_lower() {
        [0.0, -1.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    }
}

const fn canonical_zero(value: f64) -> f64 {
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
    selections: Vec<WireFaceSelectionV1>,
}

impl WireCadAuthoredGraphV1 {
    fn from_core(core: GraphCore) -> Self {
        Self {
            schema: GRAPH_SCHEMA_V1.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            length_unit: WireLengthUnit::Metre,
            requested_modeling_tolerance_m: core.requested_modeling_tolerance_m,
            sketch_plane: WireSketchPlane {
                id: SKETCH_PLANE_ID.to_owned(),
                kind: WireSketchPlaneKind::Xy,
                z_m: core.sketch.plane_z_m(),
            },
            profile: WireRectangleProfile {
                id: PROFILE_ID.to_owned(),
                kind: WireProfileKind::AxisAlignedRectangle,
                sketch_plane: SKETCH_PLANE_ID.to_owned(),
                constraint: WireConstraint::ClosedByConstruction,
                x_bounds_m: [core.sketch.x_bounds_m().0, core.sketch.x_bounds_m().1],
                y_bounds_m: [core.sketch.y_bounds_m().0, core.sketch.y_bounds_m().1],
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
                depth_m: core.extrusion_depth_m,
                repair: WireRepairDisposition::None,
            },
            selections: CadAuthoredFaceSelection::V1_ALL
                .map(|selection| {
                    WireFaceSelectionV1::try_from(selection).expect("v1 inventory is closed")
                })
                .to_vec(),
        }
    }

    fn check_contract(&self) -> Result<(), Diagnostic> {
        let expected = CadAuthoredFaceSelection::V1_ALL
            .map(|selection| WireFaceSelectionV1::try_from(selection).expect("closed v1"));
        if self.schema != GRAPH_SCHEMA_V1
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
            || self.selections.as_slice() != expected.as_slice()
        {
            return Err(invalid(
                "unsupported authored CAD v1 schema, dependency chain, selection inventory, unit, or repair disposition",
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
