use eqiora_assembly::DofId;
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_meshing::MeshEntity;
use eqiora_realization::CellCenteredConvectionScheme;
use eqiora_schema::kernel::BoundarySide;

use super::api::ScalarTransportBoundaryRole;
use crate::canonical_transport::{
    ScalarTransportCartesianBoundary, ScalarTransportCartesianModel2d,
};
use eqiora_meshing::CartesianMesh;

const DIMENSION: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ReconstructionSummary {
    pub(super) maximum_courant_number: f64,
    pub(super) limited_face_count: usize,
    pub(super) bounds: [f64; 2],
}

/// Validated per-step context that constructs every advective face trace.
pub(super) struct FaceReconstructor<'a> {
    model: &'a ScalarTransportCartesianModel2d,
    mesh: &'a CartesianMesh,
    centers: &'a [[f64; 2]],
    previous: &'a [f64],
    scheme: CellCenteredConvectionScheme,
    bounds: [f64; 2],
    maximum_courant_number: f64,
    limited_face_count: usize,
}

impl<'a> FaceReconstructor<'a> {
    pub(super) fn new(
        model: &'a ScalarTransportCartesianModel2d,
        mesh: &'a CartesianMesh,
        centers: &'a [[f64; 2]],
        previous: &'a [f64],
        scheme: CellCenteredConvectionScheme,
        duration: f64,
    ) -> Result<Self, Diagnostic> {
        if previous.len() != centers.len() {
            return Err(invalid_reconstruction(
                "transport reconstruction state and cell geometry have different shapes",
            ));
        }
        Ok(Self {
            model,
            mesh,
            centers,
            previous,
            scheme,
            bounds: reconstruction_bounds(model, previous)?,
            maximum_courant_number: validate_envelope(model, mesh, scheme, duration)?,
            limited_face_count: 0,
        })
    }

    pub(super) fn interior_trace(
        &mut self,
        axis: usize,
        lower: usize,
        upper: usize,
        outward_from_lower_flux: f64,
    ) -> Result<AffineFaceTrace, Diagnostic> {
        if self.scheme == CellCenteredConvectionScheme::ImplicitFirstOrderUpwind
            || outward_from_lower_flux == 0.0
        {
            return Ok(AffineFaceTrace::cell(if outward_from_lower_flux >= 0.0 {
                lower
            } else {
                upper
            }));
        }
        let (donor, downstream, upstream_step, upstream_side) = if outward_from_lower_flux >= 0.0 {
            (lower, upper, -1_isize, BoundarySide::Lower)
        } else {
            (upper, lower, 1_isize, BoundarySide::Upper)
        };
        let mut upstream_index = self
            .mesh
            .cell_multi_index(MeshEntity::new(DIMENSION, donor))
            .ok_or_else(|| invalid_reconstruction("transport donor cell index is unavailable"))?
            .to_vec();
        let coordinate = upstream_index[axis] as isize + upstream_step;
        let upstream = if coordinate >= 0
            && coordinate
                < self
                    .mesh
                    .axis_cell_count(axis)
                    .expect("transport axis exists") as isize
        {
            upstream_index[axis] = coordinate as usize;
            let upstream = self
                .mesh
                .cell_at(&upstream_index)
                .expect("validated Cartesian upstream index");
            PreviousStateUpstream::Cell(upstream.index())
        } else {
            let mut boundary_point = self.centers[donor];
            boundary_point[axis] = match upstream_side {
                BoundarySide::Lower => self.model.bounds()[axis][0],
                BoundarySide::Upper => self.model.bounds()[axis][1],
            };
            let Some(ScalarTransportCartesianBoundary::PrescribedTrace(value)) =
                self.model.boundary(axis, upstream_side)
            else {
                return Err(invalid_realization(
                    "Cartesian minmod upstream closure requires the exact inflow trace",
                ));
            };
            PreviousStateUpstream::InflowGhost {
                trace: value.evaluate(&boundary_point)?,
                donor,
            }
        };
        let (trace, active) = AffineFaceTrace::previous_minmod_interior(
            upstream,
            donor,
            downstream,
            self.previous,
            self.bounds,
        )?;
        self.limited_face_count += usize::from(active);
        Ok(trace)
    }

    pub(super) fn boundary_trace(
        &mut self,
        axis: usize,
        side: BoundarySide,
        cell: usize,
        role: ScalarTransportBoundaryRole,
    ) -> Result<AffineFaceTrace, Diagnostic> {
        if self.scheme == CellCenteredConvectionScheme::ImplicitFirstOrderUpwind
            || role == ScalarTransportBoundaryRole::ImpermeableWall
        {
            return Ok(AffineFaceTrace::cell(cell));
        }
        let mut upstream_index = self
            .mesh
            .cell_multi_index(MeshEntity::new(DIMENSION, cell))
            .ok_or_else(|| invalid_reconstruction("transport boundary cell index is unavailable"))?
            .to_vec();
        upstream_index[axis] = match side {
            BoundarySide::Lower => upstream_index[axis] + 1,
            BoundarySide::Upper => upstream_index[axis].checked_sub(1).ok_or_else(|| {
                invalid_reconstruction("transport outflow cell has no Cartesian upstream neighbor")
            })?,
        };
        let upstream = self.mesh.cell_at(&upstream_index).ok_or_else(|| {
            invalid_reconstruction("transport outflow upstream cell is unavailable")
        })?;
        let (trace, active) = AffineFaceTrace::previous_minmod_outflow(
            upstream.index(),
            cell,
            self.previous,
            self.bounds,
        )?;
        self.limited_face_count += usize::from(active);
        Ok(trace)
    }

    pub(super) const fn summary(&self) -> ReconstructionSummary {
        ReconstructionSummary {
            maximum_courant_number: self.maximum_courant_number,
            limited_face_count: self.limited_face_count,
            bounds: self.bounds,
        }
    }
}

/// Sparse affine trace retained independently of assembly storage.
///
/// Endpoint-implicit donor cell values are represented by `terms`; an exact
/// boundary value or previous-state MUSCL value is represented by `offset`.
/// This one contract is interpreted separately by assembly and replay.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AffineFaceTrace {
    terms: Vec<(DofId, f64)>,
    offset: f64,
    provenance: FaceTraceProvenance,
}

#[derive(Debug, Clone, PartialEq)]
enum FaceTraceProvenance {
    EndpointDonor {
        cell: usize,
    },
    ExactBoundary {
        value: f64,
    },
    PreviousStateMinmodInterior {
        upstream: PreviousStateUpstream,
        donor: usize,
        downstream: usize,
        bounds: [f64; 2],
    },
    PreviousStateMinmodOutflow {
        upstream: usize,
        donor: usize,
        bounds: [f64; 2],
    },
}

#[derive(Debug, Clone, PartialEq)]
enum PreviousStateUpstream {
    Cell(usize),
    InflowGhost { trace: f64, donor: usize },
}

struct ReplayedAffineTrace {
    terms: Vec<(DofId, f64)>,
    offset: f64,
}

impl AffineFaceTrace {
    pub(super) fn cell(cell: usize) -> Self {
        Self {
            terms: vec![(DofId::new(cell), 1.0)],
            offset: 0.0,
            provenance: FaceTraceProvenance::EndpointDonor { cell },
        }
    }

    pub(super) fn exact_boundary(value: f64) -> Result<Self, Diagnostic> {
        if !value.is_finite() {
            return Err(invalid_reconstruction(
                "cell-centered face trace must be finite",
            ));
        }
        Ok(Self {
            terms: Vec::new(),
            offset: value,
            provenance: FaceTraceProvenance::ExactBoundary { value },
        })
    }

    fn previous_minmod_interior(
        upstream: PreviousStateUpstream,
        donor: usize,
        downstream: usize,
        previous: &[f64],
        bounds: [f64; 2],
    ) -> Result<(Self, bool), Diagnostic> {
        let upstream_value = upstream.evaluate(previous)?;
        let (offset, active) = minmod_downstream_trace(
            upstream_value,
            previous_value(previous, donor)?,
            previous_value(previous, downstream)?,
            bounds,
        )?;
        Ok((
            Self {
                terms: Vec::new(),
                offset,
                provenance: FaceTraceProvenance::PreviousStateMinmodInterior {
                    upstream,
                    donor,
                    downstream,
                    bounds,
                },
            },
            active,
        ))
    }

    fn previous_minmod_outflow(
        upstream: usize,
        donor: usize,
        previous: &[f64],
        bounds: [f64; 2],
    ) -> Result<(Self, bool), Diagnostic> {
        let (offset, active) = limited_outflow_trace(
            previous_value(previous, upstream)?,
            previous_value(previous, donor)?,
            bounds,
        )?;
        Ok((
            Self {
                terms: Vec::new(),
                offset,
                provenance: FaceTraceProvenance::PreviousStateMinmodOutflow {
                    upstream,
                    donor,
                    bounds,
                },
            },
            active,
        ))
    }

    pub(super) fn terms(&self) -> &[(DofId, f64)] {
        &self.terms
    }

    pub(super) const fn offset(&self) -> f64 {
        self.offset
    }

    pub(super) fn replay_value(
        &self,
        endpoint: &[f64],
        previous: &[f64],
    ) -> Result<f64, Diagnostic> {
        let replayed = self.replayed_affine(previous)?;
        self.require_matches(&replayed)?;
        let mut value = replayed.offset;
        for &(cell, coefficient) in &replayed.terms {
            value += coefficient * previous_value(endpoint, cell.index())?;
        }
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid_reconstruction(
                "cell-centered face trace evaluation is non-finite",
            ))
        }
    }

    pub(super) fn replayed_terms(
        &self,
        previous: &[f64],
    ) -> Result<(Vec<(DofId, f64)>, f64), Diagnostic> {
        let replayed = self.replayed_affine(previous)?;
        self.require_matches(&replayed)?;
        Ok((replayed.terms, replayed.offset))
    }

    fn replayed_affine(&self, previous: &[f64]) -> Result<ReplayedAffineTrace, Diagnostic> {
        let (terms, offset) = match &self.provenance {
            FaceTraceProvenance::EndpointDonor { cell } => (vec![(DofId::new(*cell), 1.0)], 0.0),
            FaceTraceProvenance::ExactBoundary { value } => (Vec::new(), *value),
            FaceTraceProvenance::PreviousStateMinmodInterior {
                upstream,
                donor,
                downstream,
                bounds,
            } => (
                Vec::new(),
                minmod_downstream_trace(
                    upstream.evaluate(previous)?,
                    previous_value(previous, *donor)?,
                    previous_value(previous, *downstream)?,
                    *bounds,
                )?
                .0,
            ),
            FaceTraceProvenance::PreviousStateMinmodOutflow {
                upstream,
                donor,
                bounds,
            } => (
                Vec::new(),
                limited_outflow_trace(
                    previous_value(previous, *upstream)?,
                    previous_value(previous, *donor)?,
                    *bounds,
                )?
                .0,
            ),
        };
        Ok(ReplayedAffineTrace { terms, offset })
    }

    fn require_matches(&self, replayed: &ReplayedAffineTrace) -> Result<(), Diagnostic> {
        if self.terms != replayed.terms || self.offset.to_bits() != replayed.offset.to_bits() {
            return Err(invalid_reconstruction(
                "retained face trace differs from its typed evaluation provenance replay",
            ));
        }
        Ok(())
    }
}

impl PreviousStateUpstream {
    fn evaluate(&self, previous: &[f64]) -> Result<f64, Diagnostic> {
        match *self {
            Self::Cell(cell) => previous_value(previous, cell),
            Self::InflowGhost { trace, donor } => {
                Ok(2.0 * trace - previous_value(previous, donor)?)
            }
        }
    }
}

fn previous_value(values: &[f64], cell: usize) -> Result<f64, Diagnostic> {
    values.get(cell).copied().ok_or_else(|| {
        invalid_reconstruction("face trace references a cell outside its retained state")
    })
}

fn reconstruction_bounds(
    model: &ScalarTransportCartesianModel2d,
    previous: &[f64],
) -> Result<[f64; 2], Diagnostic> {
    let mut minimum = previous.iter().copied().fold(f64::INFINITY, f64::min);
    let mut maximum = previous.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut point = [
        0.5 * (model.bounds()[0][0] + model.bounds()[0][1]),
        0.5 * (model.bounds()[1][0] + model.bounds()[1][1]),
    ];
    for axis in 0..DIMENSION {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            let Some(ScalarTransportCartesianBoundary::PrescribedTrace(value)) =
                model.boundary(axis, side)
            else {
                continue;
            };
            point[axis] = match side {
                BoundarySide::Lower => model.bounds()[axis][0],
                BoundarySide::Upper => model.bounds()[axis][1],
            };
            let trace = value.evaluate(&point)?;
            minimum = minimum.min(trace);
            maximum = maximum.max(trace);
            point[axis] = 0.5 * (model.bounds()[axis][0] + model.bounds()[axis][1]);
        }
    }
    if minimum.is_finite() && maximum.is_finite() && minimum <= maximum {
        Ok([minimum, maximum])
    } else {
        Err(invalid_reconstruction(
            "transport reconstruction hull is empty or non-finite",
        ))
    }
}

fn validate_envelope(
    model: &ScalarTransportCartesianModel2d,
    mesh: &CartesianMesh,
    scheme: CellCenteredConvectionScheme,
    duration: f64,
) -> Result<f64, Diagnostic> {
    let Some(maximum_admitted) = scheme.maximum_explicit_courant_number() else {
        return Ok(0.0);
    };
    let point = [
        0.5 * (model.bounds()[0][0] + model.bounds()[0][1]),
        0.5 * (model.bounds()[1][0] + model.bounds()[1][1]),
    ];
    let velocity = model.advecting_velocity(&point)?;
    let active = velocity
        .iter()
        .enumerate()
        .filter(|(_, value)| **value != 0.0)
        .collect::<Vec<_>>();
    if active.len() > 1 {
        return Err(invalid_realization(
            "Cartesian minmod transport currently requires flow along at most one mesh axis",
        ));
    }
    let Some(&(axis, speed)) = active.first() else {
        return Ok(0.0);
    };
    let coordinates = mesh
        .axis_coordinates(axis)
        .ok_or_else(|| invalid_reconstruction("transport reconstruction axis is unavailable"))?;
    if coordinates.len() <= scheme.minimum_cells_per_active_axis() {
        return Err(invalid_realization(format!(
            "convection scheme {scheme:?} requires at least {} cells along its active Cartesian axis",
            scheme.minimum_cells_per_active_axis()
        )));
    }
    let width = coordinates[1] - coordinates[0];
    let maximum_courant_number = duration * speed.abs() / width;
    if !maximum_courant_number.is_finite()
        || maximum_courant_number > maximum_admitted + 128.0 * f64::EPSILON
    {
        return Err(invalid_realization(format!(
            "Cartesian minmod explicit convection requires Courant number <= {maximum_admitted}, received {maximum_courant_number:e}"
        )));
    }
    Ok(maximum_courant_number)
}

/// Minmod MUSCL value at the downstream face of an upstream--donor--downstream
/// triplet, evaluated entirely from the accepted previous state.
pub(super) fn minmod_downstream_trace(
    upstream: f64,
    donor: f64,
    downstream: f64,
    bounds: [f64; 2],
) -> Result<(f64, bool), Diagnostic> {
    if [upstream, donor, downstream, bounds[0], bounds[1]]
        .into_iter()
        .any(|value| !value.is_finite())
        || bounds[0] > bounds[1]
    {
        return Err(invalid_reconstruction(
            "minmod reconstruction received non-finite values or invalid bounds",
        ));
    }
    let backward = donor - upstream;
    let forward = downstream - donor;
    let unlimited = 0.5 * (backward + forward);
    let limited = minmod(backward, forward);
    let raw = donor + 0.5 * limited;
    let trace = raw.clamp(bounds[0], bounds[1]);
    let limiter_active = (limited - unlimited).abs()
        > 64.0 * f64::EPSILON * limited.abs().max(unlimited.abs()).max(1.0)
        || trace != raw;
    Ok((trace, limiter_active))
}

/// One-sided outflow extrapolation limited by the accepted global hull.
pub(super) fn limited_outflow_trace(
    upstream: f64,
    donor: f64,
    bounds: [f64; 2],
) -> Result<(f64, bool), Diagnostic> {
    minmod_downstream_trace(upstream, donor, donor + (donor - upstream), bounds)
}

const fn minmod(left: f64, right: f64) -> f64 {
    if left > 0.0 && right > 0.0 {
        if left < right { left } else { right }
    } else if left < 0.0 && right < 0.0 {
        if left > right { left } else { right }
    } else {
        0.0
    }
}

fn invalid_reconstruction(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "scalar-transport-fvm-2d".to_owned(),
        "reconstruction".to_owned(),
    ]))
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message).with_graph_path(GraphPath::new([
        "realization".to_owned(),
        "scalar-transport-fvm-2d".to_owned(),
        "reconstruction".to_owned(),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minmod_is_linear_exact_and_closes_extrema() {
        let (linear, active) = minmod_downstream_trace(0.0, 1.0, 2.0, [0.0, 2.0]).unwrap();
        assert_eq!(linear, 1.5);
        assert!(!active);

        let (extremum, active) = minmod_downstream_trace(0.0, 1.0, 0.5, [0.0, 1.0]).unwrap();
        assert_eq!(extremum, 1.0);
        assert!(active);
    }

    #[test]
    fn inflow_ghost_hull_limit_is_sharp_at_courant_one_half() {
        let (outgoing, _) = minmod_downstream_trace(1.5, 0.5, -0.5, [-0.5, 1.0]).unwrap();
        assert_eq!(outgoing, 0.0);
        let update = |courant: f64| 0.5 - courant * (outgoing - 1.0);
        assert_eq!(update(0.5), 1.0);
        assert!(update(0.5 + 1.0e-6) > 1.0);
    }

    #[test]
    fn typed_trace_replay_rejects_retained_affine_drift() {
        let previous = [0.0, 0.5, 1.0];
        let (mut trace, _) = AffineFaceTrace::previous_minmod_interior(
            PreviousStateUpstream::Cell(0),
            1,
            2,
            &previous,
            [0.0, 1.0],
        )
        .unwrap();
        trace.offset += 0.25;
        assert!(trace.replayed_terms(&previous).is_err());
        assert!(trace.replay_value(&previous, &previous).is_err());
    }
}
