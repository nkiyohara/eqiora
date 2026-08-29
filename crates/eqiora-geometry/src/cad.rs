//! A deliberately closed first CAD contract.
//!
//! This module does not model a general B-rep or feature-history tree. It
//! describes one exact operation graph whose output can be compared with the
//! existing Cartesian Geometry Identity contract:
//!
//! ```text
//! one STEP stock intersected with one fully constrained XY rectangle extrusion
//! ```
//!
//! CAD-kernel objects and enumeration indices cannot cross this boundary.

use core::fmt;

use eqiora_core::Diagnostic;
use eqiora_core::Id;
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use sha2::{Digest, Sha256};

use crate::GeometrySolidOperation;

/// Raw SHA-256 of the complete STEP byte stream consumed by a CAD adapter.
///
/// This is deliberately distinct from a canonical artifact digest. It has no
/// domain prefix and identifies source bytes, not Eqiora meaning.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StepSourceDigest([u8; 32]);

impl StepSourceDigest {
    /// Hash a complete logical STEP source byte stream.
    #[must_use]
    pub fn from_source_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Construct from already computed SHA-256 bytes.
    #[must_use]
    pub const fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Complete SHA-256 bytes.
    #[must_use]
    pub const fn sha256_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Explicit length unit declared by the admitted STEP source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepLengthUnitV1 {
    /// Unprefixed SI metre.
    Metre,
    /// SI milli-metre, converted to coherent SI by exactly `1e-3`.
    Millimetre,
}

impl StepLengthUnitV1 {
    /// Exact multiplier from source coordinates to coherent-SI metres.
    #[must_use]
    pub const fn metres_per_source_unit(self) -> f64 {
        match self {
            Self::Metre => 1.0,
            Self::Millimetre => 1.0e-3,
        }
    }
}

/// Finite, non-empty axis-aligned three-dimensional bounds in coherent SI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisAlignedBox3 {
    bounds_m: [(f64, f64); 3],
}

impl AxisAlignedBox3 {
    /// Validate and canonicalize three axis intervals.
    ///
    /// # Errors
    /// Returns `EQ0901` unless every bound is finite and strictly increasing.
    pub fn new(bounds_m: [(f64, f64); 3]) -> Result<Self, Diagnostic> {
        if bounds_m
            .iter()
            .any(|&(lower, upper)| !lower.is_finite() || !upper.is_finite() || lower >= upper)
        {
            return Err(invalid_cad(
                "CAD box bounds must be finite and strictly increasing in metres",
            ));
        }
        Ok(Self {
            bounds_m: bounds_m.map(|(lower, upper)| (canonical_zero(lower), canonical_zero(upper))),
        })
    }

    /// Canonical x/y/z intervals in metres.
    #[must_use]
    pub const fn bounds_m(self) -> [(f64, f64); 3] {
        self.bounds_m
    }

    /// Exact intersection of two axis-aligned boxes.
    ///
    /// # Errors
    /// Returns `EQ0901` when their intersection has no positive volume.
    pub fn intersection(self, other: Self) -> Result<Self, Diagnostic> {
        let mut bounds = [(0.0, 0.0); 3];
        for (axis, output) in bounds.iter_mut().enumerate() {
            *output = (
                self.bounds_m[axis].0.max(other.bounds_m[axis].0),
                self.bounds_m[axis].1.min(other.bounds_m[axis].1),
            );
        }
        Self::new(bounds).map_err(|_| {
            invalid_cad("CAD stock and extrusion must have a positive-volume intersection")
        })
    }
}

/// A fully constrained rectangle on one exact `z = constant` sketch plane.
///
/// The four finite values are the complete constraint solution: the type has
/// no free degree of freedom and therefore cannot represent an
/// under-constrained sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstrainedRectangleV1 {
    x_bounds_m: (f64, f64),
    y_bounds_m: (f64, f64),
    plane_z_m: f64,
}

impl ConstrainedRectangleV1 {
    /// Close an axis-aligned XY rectangle at an exact z coordinate.
    ///
    /// # Errors
    /// Returns `EQ0901` for non-finite or non-positive dimensions.
    pub fn new(
        x_bounds_m: (f64, f64),
        y_bounds_m: (f64, f64),
        plane_z_m: f64,
    ) -> Result<Self, Diagnostic> {
        if !plane_z_m.is_finite()
            || [x_bounds_m, y_bounds_m]
                .iter()
                .any(|&(lower, upper)| !lower.is_finite() || !upper.is_finite() || lower >= upper)
        {
            return Err(invalid_cad(
                "CAD rectangle constraints must be finite with positive width and height",
            ));
        }
        Ok(Self {
            x_bounds_m: canonical_pair(x_bounds_m),
            y_bounds_m: canonical_pair(y_bounds_m),
            plane_z_m: canonical_zero(plane_z_m),
        })
    }

    /// Exact x interval in metres.
    #[must_use]
    pub const fn x_bounds_m(self) -> (f64, f64) {
        self.x_bounds_m
    }

    /// Exact y interval in metres.
    #[must_use]
    pub const fn y_bounds_m(self) -> (f64, f64) {
        self.y_bounds_m
    }

    /// Exact sketch-plane z coordinate in metres.
    #[must_use]
    pub const fn plane_z_m(self) -> f64 {
        self.plane_z_m
    }

    /// Number of remaining sketch degrees of freedom.
    #[must_use]
    pub const fn remaining_degrees_of_freedom(self) -> usize {
        0
    }

    pub(crate) fn extruded_box(self, depth_m: f64) -> Result<AxisAlignedBox3, Diagnostic> {
        if !depth_m.is_finite() || depth_m <= 0.0 {
            return Err(invalid_cad(
                "CAD extrusion depth must be finite and positive in metres",
            ));
        }
        AxisAlignedBox3::new([
            self.x_bounds_m,
            self.y_bounds_m,
            (self.plane_z_m, self.plane_z_m + depth_m),
        ])
    }
}

/// One bounded, content-bound CAD operation graph.
///
/// V1 imports one expected axis-aligned STEP stock, extrudes one fully
/// constrained XY rectangle along positive z, and intersects the two solids.
/// Its target is an exact Semantic Domain; no source entity rank or kernel
/// face ID participates in selection.
#[derive(Clone, Debug, PartialEq)]
pub struct CadBoxDesignV1 {
    target_body: Id<kinds::Domain>,
    source: StepSourceDigest,
    source_length_unit: StepLengthUnitV1,
    imported_stock: AxisAlignedBox3,
    authoring: GeometrySolidOperation,
    source_uncertainty_m: f64,
    output: AxisAlignedBox3,
}

impl CadBoxDesignV1 {
    /// Construct the exact STEP-stock/intersection program.
    ///
    /// The declared STEP length unit is converted explicitly to coherent SI.
    /// Source uncertainty and modeling tolerance remain distinct policies and
    /// both enter design identity.
    ///
    /// # Errors
    /// Returns `EQ0901` for invalid tolerances, extrusion, or empty boolean
    /// output.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_body: Id<kinds::Domain>,
        source: StepSourceDigest,
        source_length_unit: StepLengthUnitV1,
        imported_stock: AxisAlignedBox3,
        sketch: ConstrainedRectangleV1,
        extrusion_depth_m: f64,
        source_uncertainty_m: f64,
        modeling_tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        if !source_uncertainty_m.is_finite() || source_uncertainty_m <= 0.0 {
            return Err(invalid_cad(
                "STEP source uncertainty must be finite and positive in metres",
            ));
        }
        let graph = crate::GeometryGraph::new();
        let authoring = graph.rectangle_extrusion(
            sketch.x_bounds_m(),
            sketch.y_bounds_m(),
            sketch.plane_z_m(),
            extrusion_depth_m,
            modeling_tolerance_m,
        )?;
        let output = imported_stock.intersection(authoring.output())?;
        Ok(Self {
            target_body,
            source,
            source_length_unit,
            imported_stock,
            authoring,
            source_uncertainty_m: canonical_zero(source_uncertainty_m),
            output,
        })
    }

    /// Exact Semantic body Domain receiving the result.
    #[must_use]
    pub const fn target_body(&self) -> Id<kinds::Domain> {
        self.target_body
    }

    /// Complete raw STEP source identity.
    #[must_use]
    pub const fn source(&self) -> StepSourceDigest {
        self.source
    }

    /// Exact length unit required from the STEP source.
    #[must_use]
    pub const fn source_length_unit(&self) -> StepLengthUnitV1 {
        self.source_length_unit
    }

    /// Expected normalized bounds of the imported stock.
    #[must_use]
    pub const fn imported_stock(&self) -> AxisAlignedBox3 {
        self.imported_stock
    }

    /// Fully constrained rectangle feature.
    #[must_use]
    pub const fn sketch(&self) -> ConstrainedRectangleV1 {
        self.authoring.sketch()
    }

    /// Positive-z extrusion depth in metres.
    #[must_use]
    pub const fn extrusion_depth_m(&self) -> f64 {
        self.authoring.extrusion_depth_m()
    }

    /// Declared uncertainty of the STEP source in metres.
    #[must_use]
    pub const fn source_uncertainty_m(&self) -> f64 {
        self.source_uncertainty_m
    }

    /// CAD modeling/boolean tolerance in metres.
    #[must_use]
    pub const fn modeling_tolerance_m(&self) -> f64 {
        self.authoring.requested_modeling_tolerance_m()
    }

    /// Provider-neutral graph that solely owns rectangle, face, and extrusion
    /// meaning beneath this temporary bounded convenience surface.
    #[must_use]
    pub const fn authoring_graph(&self) -> &GeometrySolidOperation {
        &self.authoring
    }

    /// Exact mathematical result of the closed intersection program.
    #[must_use]
    pub const fn output(&self) -> AxisAlignedBox3 {
        self.output
    }
}

/// Whether an adapter changed imported topology before acceptance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadRepairDispositionV1 {
    /// The accepted input required no healing or topology repair.
    None,
}

/// Kernel-independent observation of one admitted solid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadBoxObservationV1 {
    bounds: AxisAlignedBox3,
    solid_count: usize,
    closed_shell_count: usize,
    planar_face_count: usize,
    repair: CadRepairDispositionV1,
}

impl CadBoxObservationV1 {
    /// Close the only topology admitted by this first slice.
    ///
    /// # Errors
    /// Returns `EQ0901` unless the observation is exactly one solid, one
    /// closed shell, six planar faces, and no repair.
    pub fn new(
        bounds: AxisAlignedBox3,
        solid_count: usize,
        closed_shell_count: usize,
        planar_face_count: usize,
        repair: CadRepairDispositionV1,
    ) -> Result<Self, Diagnostic> {
        if solid_count != 1 || closed_shell_count != 1 || planar_face_count != 6 {
            return Err(invalid_cad(
                "CAD v1 admits exactly one solid, one closed shell, and six planar faces",
            ));
        }
        Ok(Self {
            bounds,
            solid_count,
            closed_shell_count,
            planar_face_count,
            repair,
        })
    }

    /// Axis-aligned coherent-SI bounds derived without a face rank.
    #[must_use]
    pub const fn bounds(self) -> AxisAlignedBox3 {
        self.bounds
    }

    /// Number of solids.
    #[must_use]
    pub const fn solid_count(self) -> usize {
        self.solid_count
    }

    /// Number of closed shells.
    #[must_use]
    pub const fn closed_shell_count(self) -> usize {
        self.closed_shell_count
    }

    /// Number of planar faces.
    #[must_use]
    pub const fn planar_face_count(self) -> usize {
        self.planar_face_count
    }

    /// Explicit import repair disposition.
    #[must_use]
    pub const fn repair(self) -> CadRepairDispositionV1 {
        self.repair
    }
}

/// Complete kernel-independent observation of the three operation boundaries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadBoxRealizationV1 {
    imported_stock: CadBoxObservationV1,
    extruded_tool: CadBoxObservationV1,
    intersection: CadBoxObservationV1,
}

impl CadBoxRealizationV1 {
    /// Validate observations against the exact closed design.
    ///
    /// # Errors
    /// Returns `EQ0901` for source, extrusion, boolean, topology, or repair
    /// drift.
    pub fn new(
        design: &CadBoxDesignV1,
        imported_stock: CadBoxObservationV1,
        extruded_tool: CadBoxObservationV1,
        intersection: CadBoxObservationV1,
    ) -> Result<Self, Diagnostic> {
        let expected_tool = design.authoring.output();
        if imported_stock.bounds != design.imported_stock
            || extruded_tool.bounds != expected_tool
            || intersection.bounds != design.output
        {
            return Err(invalid_cad(
                "CAD observations differ from the exact STEP, extrusion, or intersection design",
            ));
        }
        Ok(Self {
            imported_stock,
            extruded_tool,
            intersection,
        })
    }

    /// Imported STEP stock observation.
    #[must_use]
    pub const fn imported_stock(self) -> CadBoxObservationV1 {
        self.imported_stock
    }

    /// Constrained-sketch extrusion observation.
    #[must_use]
    pub const fn extruded_tool(self) -> CadBoxObservationV1 {
        self.extruded_tool
    }

    /// Boolean-intersection result observation.
    #[must_use]
    pub const fn intersection(self) -> CadBoxObservationV1 {
        self.intersection
    }
}

/// Exact inspectable identity of a compile-time CAD adapter and its kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CadAdapterIdentityV1 {
    adapter: &'static str,
    adapter_version: &'static str,
    kernel: &'static str,
    kernel_version: &'static str,
}

impl CadAdapterIdentityV1 {
    /// Construct a static identity supplied by adapter code, never source.
    #[must_use]
    pub const fn new(
        adapter: &'static str,
        adapter_version: &'static str,
        kernel: &'static str,
        kernel_version: &'static str,
    ) -> Self {
        Self {
            adapter,
            adapter_version,
            kernel,
            kernel_version,
        }
    }

    /// Stable adapter ID.
    #[must_use]
    pub const fn adapter(self) -> &'static str {
        self.adapter
    }

    /// Exact adapter implementation version.
    #[must_use]
    pub const fn adapter_version(self) -> &'static str {
        self.adapter_version
    }

    /// Geometry-kernel implementation ID.
    #[must_use]
    pub const fn kernel(self) -> &'static str {
        self.kernel
    }

    /// Exact kernel version admitted by this adapter build.
    #[must_use]
    pub const fn kernel_version(self) -> &'static str {
        self.kernel_version
    }
}

/// Compile-time seam implemented by an isolated CAD-kernel adapter.
///
/// The adapter receives complete source bytes and may return only Eqiora-owned
/// observations. Runtime loading, opaque callbacks, kernel objects, and face
/// indices are intentionally absent.
pub trait CadKernelAdapter {
    /// Exact adapter and kernel build identity.
    fn identity(&self) -> CadAdapterIdentityV1;

    /// Replay the closed design against complete STEP bytes.
    ///
    /// # Errors
    /// Must fail closed for digest drift, unsupported STEP topology or unit,
    /// repair, non-planar/non-axis-aligned output, or boolean failure.
    fn realize_box_design(
        &self,
        design: &CadBoxDesignV1,
        step_bytes: &[u8],
    ) -> Result<CadBoxRealizationV1, Diagnostic>;
}

fn canonical_pair(pair: (f64, f64)) -> (f64, f64) {
    (canonical_zero(pair.0), canonical_zero(pair.1))
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn invalid_cad(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

impl fmt::Display for StepSourceDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box3(bounds: [(f64, f64); 3]) -> AxisAlignedBox3 {
        AxisAlignedBox3::new(bounds).unwrap()
    }

    #[test]
    fn closes_exact_stock_extrusion_intersection_without_free_sketch_dofs() {
        let sketch = ConstrainedRectangleV1::new((0.25, 1.75), (0.0, 1.0), 0.0).unwrap();
        let design = CadBoxDesignV1::new(
            Id::new(),
            StepSourceDigest::from_sha256([7; 32]),
            StepLengthUnitV1::Metre,
            box3([(0.0, 2.0), (0.0, 1.0), (0.0, 1.0)]),
            sketch,
            1.0,
            1.0e-12,
            1.0e-10,
        )
        .unwrap();

        assert_eq!(sketch.remaining_degrees_of_freedom(), 0);
        assert_eq!(
            design.output().bounds_m(),
            [(0.25, 1.75), (0.0, 1.0), (0.0, 1.0)]
        );
    }

    #[test]
    fn rejects_nonfinite_degenerate_and_disjoint_designs() {
        assert!(AxisAlignedBox3::new([(0.0, 0.0), (0.0, 1.0), (0.0, 1.0)]).is_err());
        assert!(ConstrainedRectangleV1::new((0.0, f64::INFINITY), (0.0, 1.0), 0.0).is_err());

        let result = CadBoxDesignV1::new(
            Id::new(),
            StepSourceDigest::from_sha256([0; 32]),
            StepLengthUnitV1::Metre,
            box3([(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)]),
            ConstrainedRectangleV1::new((2.0, 3.0), (0.0, 1.0), 0.0).unwrap(),
            1.0,
            1.0e-12,
            1.0e-10,
        );
        assert!(result.is_err());
    }

    #[test]
    fn observation_rejects_topology_and_design_drift() {
        let bounds = box3([(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)]);
        assert!(CadBoxObservationV1::new(bounds, 1, 1, 5, CadRepairDispositionV1::None).is_err());

        let design = CadBoxDesignV1::new(
            Id::new(),
            StepSourceDigest::from_sha256([1; 32]),
            StepLengthUnitV1::Metre,
            bounds,
            ConstrainedRectangleV1::new((0.0, 1.0), (0.0, 1.0), 0.0).unwrap(),
            1.0,
            1.0e-12,
            1.0e-10,
        )
        .unwrap();
        let accepted =
            CadBoxObservationV1::new(bounds, 1, 1, 6, CadRepairDispositionV1::None).unwrap();
        let drifted = CadBoxObservationV1::new(
            box3([(0.0, 0.5), (0.0, 1.0), (0.0, 1.0)]),
            1,
            1,
            6,
            CadRepairDispositionV1::None,
        )
        .unwrap();
        assert!(CadBoxRealizationV1::new(&design, accepted, accepted, drifted).is_err());
    }
}
