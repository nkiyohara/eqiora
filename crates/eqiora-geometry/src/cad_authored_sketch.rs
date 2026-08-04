//! Closed native inputs for the two admitted authored-CAD operations.
//!
//! This opaque owner is intentionally not a general sketch, profile, plane,
//! constraint, or operation graph. It admits only the rectangle extrusion and
//! graph-bound circular through-cut already represented by the closed graph
//! wires.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::ConstrainedRectangleV1;
use crate::cad_authored_cut::AdmittedCircle;
use crate::cad_authored_graph::CadAuthoredGraph;
use crate::cad_authored_selection::{CadAuthoredFaceHandle, FaceKey};

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[derive(Clone, Debug, PartialEq)]
enum SketchInput {
    Rectangle {
        rectangle: ConstrainedRectangleV1,
        requested_modeling_tolerance_m: f64,
    },
    Circle {
        face: CadAuthoredFaceHandle,
        circle: AdmittedCircle,
    },
}

/// One opaque native owner for the two closed authored-CAD sketch inputs.
///
/// The private representation can contain exactly one constrained XY
/// rectangle with its modeling tolerance or one exact circle bound to a v1
/// graph's canonical `end-cap` face handle.
#[derive(Clone, Debug, PartialEq)]
pub struct CadAuthoredSketch {
    input: SketchInput,
}

impl CadAuthoredSketch {
    /// Admit the one constrained rectangle input on its existing XY plane.
    ///
    /// # Errors
    /// Returns `EQ0901` unless the requested modeling tolerance is finite and
    /// strictly positive. Rectangle-coordinate admission remains owned by
    /// [`ConstrainedRectangleV1::new`].
    pub fn rectangle_xy(
        rectangle: ConstrainedRectangleV1,
        requested_modeling_tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        if !requested_modeling_tolerance_m.is_finite() || requested_modeling_tolerance_m <= 0.0 {
            return Err(invalid(
                "authored CAD modeling tolerance must be finite and positive in metres",
            ));
        }
        Ok(Self {
            input: SketchInput::Rectangle {
                rectangle,
                requested_modeling_tolerance_m: canonical_zero(requested_modeling_tolerance_m),
            },
        })
    }

    /// Admit one exact circle bound to a predecessor v1 graph's end cap.
    ///
    /// Graph identity is intentionally resolved only when a target graph is
    /// supplied to [`CadAuthoredGraph::through_cut`].
    ///
    /// # Errors
    /// Returns `EQ0901` unless the handle is the canonical v1 `end-cap`, both
    /// centre coordinates are finite, and the radius is finite and strictly
    /// positive.
    pub fn circle_on_face(
        face: CadAuthoredFaceHandle,
        center_m: [f64; 2],
        radius_m: f64,
    ) -> Result<Self, Diagnostic> {
        if !face.is_v1() || face.face_key() != FaceKey::end_cap() {
            return Err(invalid(
                "authored CAD circle sketch requires a v1 end-cap face handle",
            ));
        }
        Ok(Self {
            input: SketchInput::Circle {
                face,
                circle: AdmittedCircle::new(center_m, radius_m)?,
            },
        })
    }

    /// Apply the one admitted positive-z extrusion to a rectangle sketch.
    ///
    /// # Errors
    /// Returns `EQ0901` for a circle sketch, a non-finite/non-positive depth,
    /// or a non-finite derived end plane.
    pub fn extrude_positive_z(&self, depth_m: f64) -> Result<CadAuthoredGraph, Diagnostic> {
        let SketchInput::Rectangle {
            rectangle,
            requested_modeling_tolerance_m,
        } = &self.input
        else {
            return Err(invalid(
                "positive-z extrusion requires the admitted rectangle sketch",
            ));
        };
        CadAuthoredGraph::from_rectangle_sketch(
            *rectangle,
            *requested_modeling_tolerance_m,
            depth_m,
        )
    }

    pub(crate) fn circle_parts(&self) -> Option<(&CadAuthoredFaceHandle, AdmittedCircle)> {
        match &self.input {
            SketchInput::Circle { face, circle } => Some((face, *circle)),
            SketchInput::Rectangle { .. } => None,
        }
    }
}

const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}
