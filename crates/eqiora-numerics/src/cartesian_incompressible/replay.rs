use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};

use crate::cartesian_fvm_geometry::{
    CartesianCellMetrics2d, CartesianFacetAdjacency2d, CartesianFacetMetrics2d,
};

use super::operator::{CollocatedFaceAction2d, CollocatedPoint2d, CollocatedResidual2d};

const DIMENSION: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CollocatedResidualReplay2d {
    pub(crate) maximum_momentum_defect: f64,
    pub(crate) maximum_continuity_defect: f64,
    pub(crate) maximum_face_cancellation_defect: f64,
    pub(crate) maximum_flux_reuse_defect: f64,
    pub(crate) global_mass_defect: f64,
    pub(crate) tolerance: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_residual(
    facets: &[CartesianFacetMetrics2d],
    cells: &[CartesianCellMetrics2d],
    density: f64,
    duration: f64,
    previous_velocity: &[[f64; DIMENSION]],
    body_force: &[[f64; DIMENSION]],
    point: &CollocatedPoint2d,
    residual: &CollocatedResidual2d,
) -> Result<CollocatedResidualReplay2d, Diagnostic> {
    if facets.len() != residual.face_actions.len()
        || cells.len() != point.velocity.len()
        || cells.len() != previous_velocity.len()
        || cells.len() != body_force.len()
        || residual.momentum.len() != cells.len()
        || residual.physical_continuity.len() != cells.len()
    {
        return Err(invalid(
            "collocated replay requires one retained face action per finalized facet and one value per cell",
        ));
    }

    let mut momentum = vec![[0.0; DIMENSION]; cells.len()];
    let mut continuity = vec![0.0; cells.len()];
    for (cell, cell_momentum) in momentum.iter_mut().enumerate() {
        for component in 0..DIMENSION {
            cell_momentum[component] = density * cells[cell].measure / duration
                * (point.velocity[cell][component] - previous_velocity[cell][component])
                - cells[cell].measure * body_force[cell][component];
        }
    }

    let mut maximum_face_cancellation_defect = 0.0_f64;
    let mut maximum_flux_reuse_defect = 0.0_f64;
    for (facet, action) in facets.iter().zip(&residual.face_actions) {
        match (facet.adjacency, action) {
            (
                CartesianFacetAdjacency2d::Interior { lower, upper, .. },
                CollocatedFaceAction2d::Interior {
                    lower: action_lower,
                    upper: action_upper,
                    volume_flux,
                    face_velocity,
                    convective_momentum,
                    traction_momentum,
                },
            ) if lower == *action_lower && upper == *action_upper => {
                let expected_face_velocity = [
                    0.5 * (point.velocity[lower][0] + point.velocity[upper][0]),
                    0.5 * (point.velocity[lower][1] + point.velocity[upper][1]),
                ];
                continuity[lower] += volume_flux;
                continuity[upper] -= volume_flux;
                maximum_face_cancellation_defect =
                    maximum_face_cancellation_defect.max((volume_flux + -volume_flux).abs());
                for component in 0..DIMENSION {
                    let expected_convection =
                        density * volume_flux * expected_face_velocity[component];
                    maximum_flux_reuse_defect = maximum_flux_reuse_defect
                        .max((face_velocity[component] - expected_face_velocity[component]).abs())
                        .max((convective_momentum[component] - expected_convection).abs());
                    let lower_action =
                        convective_momentum[component] - traction_momentum[component];
                    momentum[lower][component] += lower_action;
                    momentum[upper][component] -= lower_action;
                    maximum_face_cancellation_defect =
                        maximum_face_cancellation_defect.max((lower_action + -lower_action).abs());
                }
            }
            (
                CartesianFacetAdjacency2d::Boundary { cell, .. },
                CollocatedFaceAction2d::Boundary {
                    cell: action_cell,
                    traction_momentum,
                },
            ) if cell == *action_cell => {
                for component in 0..DIMENSION {
                    momentum[cell][component] -= traction_momentum[component];
                }
            }
            _ => {
                return Err(invalid(
                    "collocated retained face action differs from finalized facet kind or orientation",
                ));
            }
        }
    }

    let mut maximum_momentum_defect = 0.0_f64;
    let mut maximum_continuity_defect = 0.0_f64;
    let mut comparison_scale = 1.0_f64;
    for (replayed, accepted) in momentum.iter().zip(&residual.momentum) {
        for component in 0..DIMENSION {
            maximum_momentum_defect =
                maximum_momentum_defect.max((replayed[component] - accepted[component]).abs());
            comparison_scale = comparison_scale
                .max(replayed[component].abs())
                .max(accepted[component].abs());
        }
    }
    for (replayed, accepted) in continuity.iter().zip(&residual.physical_continuity) {
        maximum_continuity_defect = maximum_continuity_defect.max((replayed - accepted).abs());
        comparison_scale = comparison_scale.max(replayed.abs()).max(accepted.abs());
    }
    let global_mass_defect = continuity.iter().sum::<f64>().abs();
    comparison_scale = comparison_scale
        .max(maximum_flux_reuse_defect)
        .max(global_mass_defect);
    let tolerance = 4096.0 * f64::EPSILON * comparison_scale;
    if [
        maximum_momentum_defect,
        maximum_continuity_defect,
        maximum_face_cancellation_defect,
        maximum_flux_reuse_defect,
        global_mass_defect,
        tolerance,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || maximum_momentum_defect > tolerance
        || maximum_continuity_defect > tolerance
        || maximum_face_cancellation_defect > tolerance
        || maximum_flux_reuse_defect > tolerance
        || global_mass_defect > tolerance
    {
        return Err(invalid(format!(
            "collocated replay exceeded tolerance {tolerance:e}: momentum {maximum_momentum_defect:e}, continuity {maximum_continuity_defect:e}, face cancellation {maximum_face_cancellation_defect:e}, flux reuse {maximum_flux_reuse_defect:e}, global mass {global_mass_defect:e}"
        )));
    }
    Ok(CollocatedResidualReplay2d {
        maximum_momentum_defect,
        maximum_continuity_defect,
        maximum_face_cancellation_defect,
        maximum_flux_reuse_defect,
        global_mass_defect,
        tolerance,
    })
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "cartesian-incompressible-fvm-2d".to_owned(),
        "residual-replay".to_owned(),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian_incompressible::CartesianIncompressibleOperator2d;
    use eqiora_meshing::CartesianMesh;

    fn zero_problem() -> (CartesianIncompressibleOperator2d, CollocatedPoint2d) {
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[2, 2]).unwrap();
        let face_count = crate::cartesian_fvm_geometry::cartesian_fvm_geometry_2d(&mesh)
            .unwrap()
            .1
            .len();
        let point = CollocatedPoint2d {
            velocity: vec![[0.0, 0.0]; 4],
            pressure: vec![0.0; 4],
            gauge_multiplier: 0.0,
        };
        let operator = CartesianIncompressibleOperator2d::new(
            mesh,
            1.0,
            0.1,
            0.05,
            point.velocity.clone(),
            vec![0.0; face_count],
            vec![[0.0, 0.0]; 4],
        )
        .unwrap();
        (operator, point)
    }

    #[test]
    fn exact_retained_actions_replay_both_physical_blocks() {
        let (operator, point) = zero_problem();
        let residual = operator.evaluate(&point).unwrap();
        let replay = operator.replay(&point, &residual).unwrap();
        assert_eq!(replay.maximum_momentum_defect, 0.0);
        assert_eq!(replay.maximum_continuity_defect, 0.0);
        assert_eq!(replay.maximum_face_cancellation_defect, 0.0);
        assert_eq!(replay.maximum_flux_reuse_defect, 0.0);
        assert_eq!(replay.global_mass_defect, 0.0);
    }

    #[test]
    fn missing_or_reoriented_face_action_fails_closed() {
        let (operator, point) = zero_problem();
        let mut missing = operator.evaluate(&point).unwrap();
        missing.face_actions.pop();
        assert!(operator.replay(&point, &missing).is_err());

        let mut reoriented = operator.evaluate(&point).unwrap();
        let interior = reoriented
            .face_actions
            .iter_mut()
            .find(|action| matches!(action, CollocatedFaceAction2d::Interior { .. }))
            .unwrap();
        if let CollocatedFaceAction2d::Interior { lower, upper, .. } = interior {
            std::mem::swap(lower, upper);
        }
        assert!(operator.replay(&point, &reoriented).is_err());
    }
}
