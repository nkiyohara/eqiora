use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_solver::CanonicalCsrSystemView;

use super::api::{ScalarTransportBoundaryRole, TransportFace2d};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct OperatorReplay {
    pub(super) maximum_defect: f64,
    pub(super) tolerance: f64,
}

/// Reconstruct the complete scaled matrix and right-hand side without using
/// assembly packets, then compare them with the captured canonical CSR.
pub(super) fn require_complete_operator(
    system: &CanonicalCsrSystemView,
    measures: &[f64],
    faces: &[TransportFace2d],
    previous: &[f64],
    duration: f64,
    state_scale: f64,
    weak_scale: f64,
) -> Result<OperatorReplay, Diagnostic> {
    if measures.len() != previous.len() || system.rows() != measures.len() {
        return Err(invalid_numerics(
            "transport operator replay received inconsistent cell shapes",
        ));
    }
    let row_scale = state_scale / weak_scale;
    let matrix_scale = state_scale * row_scale;
    let mut rows = vec![BTreeMap::<usize, f64>::new(); measures.len()];
    let mut right_hand_side = vec![0.0; measures.len()];

    for (cell, (measure, previous)) in measures.iter().zip(previous).enumerate() {
        let mass = measure / duration;
        add(&mut rows, cell, cell, matrix_scale * mass)?;
        right_hand_side[cell] += row_scale * mass * previous;
    }
    for face in faces {
        match face {
            TransportFace2d::Interior {
                lower,
                upper,
                outward_from_lower_flux,
                transmissibility,
                advective_trace,
            } => {
                let (trace_terms, trace_offset) = advective_trace.replayed_terms(previous)?;
                add(&mut rows, *lower, *lower, matrix_scale * transmissibility)?;
                add(&mut rows, *lower, *upper, -matrix_scale * transmissibility)?;
                add(&mut rows, *upper, *lower, -matrix_scale * transmissibility)?;
                add(&mut rows, *upper, *upper, matrix_scale * transmissibility)?;
                for &(cell, coefficient) in &trace_terms {
                    add(
                        &mut rows,
                        *lower,
                        cell.index(),
                        matrix_scale * outward_from_lower_flux * coefficient,
                    )?;
                    add(
                        &mut rows,
                        *upper,
                        cell.index(),
                        -matrix_scale * outward_from_lower_flux * coefficient,
                    )?;
                }
                let explicit_flux = outward_from_lower_flux * trace_offset;
                right_hand_side[*lower] -= row_scale * explicit_flux;
                right_hand_side[*upper] += row_scale * explicit_flux;
            }
            TransportFace2d::PrescribedTrace {
                cell,
                outward_volume_flux,
                transmissibility,
                trace,
                advective_trace,
            } => {
                let (trace_terms, trace_offset) = advective_trace.replayed_terms(previous)?;
                add(&mut rows, *cell, *cell, matrix_scale * transmissibility)?;
                for &(unknown, coefficient) in &trace_terms {
                    add(
                        &mut rows,
                        *cell,
                        unknown.index(),
                        matrix_scale * outward_volume_flux * coefficient,
                    )?;
                }
                right_hand_side[*cell] +=
                    row_scale * (transmissibility * trace - outward_volume_flux * trace_offset);
            }
            TransportFace2d::PrescribedDiffusiveFlux {
                cell,
                outward_volume_flux,
                diffusive_flux_integral,
                advective_trace,
                ..
            } => {
                let (trace_terms, trace_offset) = advective_trace.replayed_terms(previous)?;
                for &(unknown, coefficient) in &trace_terms {
                    add(
                        &mut rows,
                        *cell,
                        unknown.index(),
                        matrix_scale * outward_volume_flux * coefficient,
                    )?;
                }
                right_hand_side[*cell] +=
                    row_scale * (diffusive_flux_integral - outward_volume_flux * trace_offset);
            }
        }
    }

    let mut expected_offsets = Vec::with_capacity(rows.len() + 1);
    let mut expected_columns = Vec::new();
    let mut expected_values = Vec::new();
    expected_offsets.push(0);
    for row in &rows {
        expected_columns.extend(row.keys().copied());
        expected_values.extend(row.values().copied());
        expected_offsets.push(expected_columns.len());
    }
    if system.row_offsets() != expected_offsets || system.column_indices() != expected_columns {
        return Err(invalid_numerics(
            "transport assembled CSR structure differs from the independent physical operator replay",
        ));
    }

    let mut maximum_defect: f64 = 0.0;
    let mut comparison_scale: f64 = 1.0;
    for (actual, expected) in system
        .values()
        .iter()
        .zip(&expected_values)
        .chain(system.right_hand_side().iter().zip(&right_hand_side))
    {
        maximum_defect = maximum_defect.max((actual - expected).abs());
        comparison_scale = comparison_scale.max(actual.abs()).max(expected.abs());
    }
    let tolerance = 4096.0 * f64::EPSILON * comparison_scale;
    if !maximum_defect.is_finite() || maximum_defect > tolerance {
        return Err(invalid_numerics(format!(
            "transport independent operator replay defect {maximum_defect:e} exceeds tolerance {tolerance:e}"
        )));
    }
    Ok(OperatorReplay {
        maximum_defect,
        tolerance,
    })
}

fn add(
    rows: &mut [BTreeMap<usize, f64>],
    row: usize,
    column: usize,
    value: f64,
) -> Result<(), Diagnostic> {
    if row >= rows.len() || column >= rows.len() || !value.is_finite() {
        return Err(invalid_numerics(
            "transport operator replay entry is outside the system or non-finite",
        ));
    }
    let target = &mut rows[row];
    *target.entry(column).or_default() += value;
    Ok(())
}

pub(super) fn boundary_flux(
    faces: &[TransportFace2d],
    previous: &[f64],
    values: &[f64],
) -> Result<f64, Diagnostic> {
    let mut total = 0.0;
    for face in faces {
        let flux = match face {
            TransportFace2d::Interior { .. } => continue,
            TransportFace2d::PrescribedTrace {
                cell,
                outward_volume_flux,
                transmissibility,
                trace,
                advective_trace,
            } => {
                outward_volume_flux * advective_trace.replay_value(values, previous)?
                    + transmissibility * (values[*cell] - trace)
            }
            TransportFace2d::PrescribedDiffusiveFlux {
                cell: _,
                outward_volume_flux,
                diffusive_flux_integral,
                advective_trace,
                ..
            } => {
                outward_volume_flux * advective_trace.replay_value(values, previous)?
                    - diffusive_flux_integral
            }
        };
        total += flux;
    }
    if !total.is_finite() {
        Err(invalid_numerics(
            "transport boundary flux reconstruction is non-finite",
        ))
    } else {
        Ok(total)
    }
}

pub(super) fn replay_interior_cancellation(
    faces: &[TransportFace2d],
    previous: &[f64],
    values: &[f64],
) -> Result<f64, Diagnostic> {
    let mut maximum: f64 = 0.0;
    for face in faces {
        let TransportFace2d::Interior {
            lower,
            upper,
            outward_from_lower_flux,
            transmissibility,
            advective_trace,
        } = face
        else {
            continue;
        };
        let trace = advective_trace.replay_value(values, previous)?;
        let lower_outward =
            outward_from_lower_flux * trace + transmissibility * (values[*lower] - values[*upper]);
        let upper_outward =
            -outward_from_lower_flux * trace + transmissibility * (values[*upper] - values[*lower]);
        maximum = maximum.max((lower_outward + upper_outward).abs());
    }
    if maximum.is_finite() {
        Ok(maximum)
    } else {
        Err(invalid_numerics(
            "transport interior flux cancellation replay is non-finite",
        ))
    }
}

pub(super) fn replay_physical_residual(
    measures: &[f64],
    faces: &[TransportFace2d],
    previous: &[f64],
    current: &[f64],
    duration: f64,
) -> Result<Vec<f64>, Diagnostic> {
    if measures.len() != previous.len() || previous.len() != current.len() {
        return Err(invalid_numerics(
            "transport physical replay received inconsistent cell shapes",
        ));
    }
    let mut residual = measures
        .iter()
        .zip(previous)
        .zip(current)
        .map(|((measure, previous), current)| measure * (current - previous) / duration)
        .collect::<Vec<_>>();
    for face in faces {
        match face {
            TransportFace2d::Interior {
                lower,
                upper,
                outward_from_lower_flux,
                transmissibility,
                advective_trace,
            } => {
                let trace = advective_trace.replay_value(current, previous)?;
                residual[*lower] += outward_from_lower_flux * trace
                    + transmissibility * (current[*lower] - current[*upper]);
                residual[*upper] += -outward_from_lower_flux * trace
                    + transmissibility * (current[*upper] - current[*lower]);
            }
            TransportFace2d::PrescribedTrace {
                cell,
                outward_volume_flux,
                transmissibility,
                trace,
                advective_trace,
            } => {
                residual[*cell] += outward_volume_flux
                    * advective_trace.replay_value(current, previous)?
                    + transmissibility * (current[*cell] - trace);
            }
            TransportFace2d::PrescribedDiffusiveFlux {
                cell,
                outward_volume_flux,
                diffusive_flux_integral,
                advective_trace,
                ..
            } => {
                residual[*cell] += outward_volume_flux
                    * advective_trace.replay_value(current, previous)?
                    - diffusive_flux_integral;
            }
        }
    }
    if residual.iter().any(|value| !value.is_finite()) {
        Err(invalid_numerics(
            "transport independent physical residual replay is non-finite",
        ))
    } else {
        Ok(residual)
    }
}

pub(super) fn integrated_mass(measures: &[f64], values: &[f64]) -> f64 {
    measures
        .iter()
        .zip(values)
        .map(|(measure, value)| measure * value)
        .sum()
}

pub(super) fn advective_face_range(
    faces: &[TransportFace2d],
    previous: &[f64],
    values: &[f64],
    bounds: [f64; 2],
) -> Result<(Option<[f64; 2]>, f64), Diagnostic> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    let mut bound_defect: f64 = 0.0;
    for face in faces {
        let (volume_flux, trace) = match face {
            TransportFace2d::Interior {
                outward_from_lower_flux,
                advective_trace,
                ..
            } => (
                *outward_from_lower_flux,
                advective_trace.replay_value(values, previous)?,
            ),
            TransportFace2d::PrescribedTrace {
                outward_volume_flux,
                advective_trace,
                ..
            }
            | TransportFace2d::PrescribedDiffusiveFlux {
                outward_volume_flux,
                advective_trace,
                ..
            } => (
                *outward_volume_flux,
                advective_trace.replay_value(values, previous)?,
            ),
        };
        if volume_flux == 0.0 {
            continue;
        }
        minimum = minimum.min(trace);
        maximum = maximum.max(trace);
        bound_defect = bound_defect
            .max((bounds[0] - trace).max(0.0))
            .max((trace - bounds[1]).max(0.0));
    }
    if minimum == f64::INFINITY && maximum == f64::NEG_INFINITY {
        Ok((None, 0.0))
    } else if minimum.is_finite() && maximum.is_finite() && bound_defect.is_finite() {
        Ok((Some([minimum, maximum]), bound_defect))
    } else {
        Err(invalid_numerics(
            "transport advective face range is empty or non-finite",
        ))
    }
}

pub(super) fn face_counts(faces: &[TransportFace2d]) -> (usize, usize, usize, usize, usize) {
    let mut counts = (0, 0, 0, 0, 0);
    for face in faces {
        match face {
            TransportFace2d::Interior { .. } => counts.0 += 1,
            TransportFace2d::PrescribedTrace { .. } => {
                counts.1 += 1;
                counts.2 += 1;
            }
            TransportFace2d::PrescribedDiffusiveFlux { role, .. } => {
                counts.1 += 1;
                match role {
                    ScalarTransportBoundaryRole::Outflow => counts.3 += 1,
                    ScalarTransportBoundaryRole::ImpermeableWall => counts.4 += 1,
                    ScalarTransportBoundaryRole::Inflow => {
                        unreachable!("inflow is represented by a prescribed trace")
                    }
                }
            }
        }
    }
    counts
}

fn invalid_numerics(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "scalar-transport-fvm-2d".to_owned(),
        "replay".to_owned(),
    ]))
}
