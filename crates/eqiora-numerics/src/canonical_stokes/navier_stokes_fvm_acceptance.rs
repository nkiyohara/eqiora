//! Independent acceptance checks for one collocated FVM step.

use eqiora_core::Diagnostic;
use eqiora_meshing::CartesianMesh;

use crate::cartesian_incompressible::{
    CartesianIncompressibleOperator2d, CollocatedNewtonEvidence2d, CollocatedPoint2d,
    CollocatedResidual2d,
};

use super::navier_stokes_fvm_realization::CellCenteredNavierStokesStepEvidence2d;

const DIMENSION: usize = 2;

pub(super) fn accept_collocated_step(
    mesh: &CartesianMesh,
    cells: &[crate::cartesian_fvm_geometry::CartesianCellMetrics2d],
    operator: &CartesianIncompressibleOperator2d,
    accepted: &CollocatedPoint2d,
    residual: &CollocatedResidual2d,
    newton: CollocatedNewtonEvidence2d,
) -> Result<CellCenteredNavierStokesStepEvidence2d, Diagnostic> {
    let replay = operator.replay(accepted, residual)?;
    let constant = vec![1.0; operator.cell_count()];
    let affine = cells
        .iter()
        .map(|cell| 1.0 + 2.0 * cell.center[0] - 3.0 * cell.center[1])
        .collect::<Vec<_>>();
    let maximum_affine_pressure_correction = constant
        .iter()
        .map(|_| 0.0)
        .chain(operator.pressure_corrections(&constant)?)
        .chain(operator.pressure_corrections(&affine)?)
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    let mut checkerboard_norms = Vec::new();
    for axes in [[true, false], [false, true], [true, true]] {
        let pressure = (0..operator.cell_count())
            .map(|cell| {
                let indices = mesh
                    .cell_multi_index(eqiora_meshing::MeshEntity::new(DIMENSION, cell))
                    .expect("accepted Cartesian cell owns its multi-index");
                let parity = usize::from(axes[0]) * indices[0] + usize::from(axes[1]) * indices[1];
                if parity & 1 == 0 { 1.0 } else { -1.0 }
            })
            .collect::<Vec<_>>();
        let squared = operator
            .pressure_corrections(&pressure)?
            .iter()
            .map(|value| value * value)
            .sum::<f64>();
        checkerboard_norms.push(squared.sqrt());
    }
    let minimum_checkerboard_correction_norm =
        checkerboard_norms.into_iter().fold(f64::INFINITY, f64::min);
    let scale = operator
        .momentum_diagonal()
        .iter()
        .flatten()
        .copied()
        .fold(1.0_f64, f64::max);
    let affine_tolerance = 1024.0 * f64::EPSILON * scale;
    require_pressure_coupling_evidence(
        maximum_affine_pressure_correction,
        minimum_checkerboard_correction_norm,
        affine_tolerance,
    )?;
    if newton.maximum_centered_jvp_defect > 2.0e-7
        || residual.momentum_norm > newton.momentum_target
        || residual.continuity_norm > newton.continuity_target
        || residual.gauge_residual.abs() > newton.gauge_target
    {
        return Err(invalid_realization(
            "collocated step failed residual, JVP, affine-pressure, checkerboard, gauge, or face-cancellation acceptance",
        ));
    }
    Ok(CellCenteredNavierStokesStepEvidence2d {
        iterations: newton.iterations,
        initial_residual_norm: newton.initial_residual_norm,
        initial_momentum_norm: newton.initial_momentum_norm,
        initial_continuity_norm: newton.initial_continuity_norm,
        residual_target: newton.residual_target,
        momentum_residual_target: newton.momentum_target,
        continuity_residual_target: newton.continuity_target,
        gauge_residual_target: newton.gauge_target,
        momentum_residual_norm: residual.momentum_norm,
        continuity_residual_norm: residual.continuity_norm,
        gauge_residual: residual.gauge_residual,
        maximum_momentum_replay_defect: replay.maximum_momentum_defect,
        maximum_continuity_replay_defect: replay.maximum_continuity_defect,
        maximum_centered_jvp_defect: newton.maximum_centered_jvp_defect,
        maximum_face_cancellation_defect: replay.maximum_face_cancellation_defect,
        maximum_flux_reuse_defect: replay.maximum_flux_reuse_defect,
        global_mass_defect: replay.global_mass_defect,
        replay_tolerance: replay.tolerance,
        maximum_affine_pressure_correction,
        minimum_checkerboard_correction_norm,
        linear_solves: newton.linear_solves,
    })
}

fn require_pressure_coupling_evidence(
    maximum_affine_correction: f64,
    minimum_checkerboard_action: f64,
    affine_tolerance: f64,
) -> Result<(), Diagnostic> {
    if !maximum_affine_correction.is_finite()
        || maximum_affine_correction > affine_tolerance
        || !minimum_checkerboard_action.is_finite()
        || minimum_checkerboard_action <= 1024.0 * f64::EPSILON
    {
        Err(invalid_realization(
            "collocated pressure coupling failed affine exactness or admitted an unstabilized checkerboard null action",
        ))
    } else {
        Ok(())
    }
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(eqiora_core::diagnostic::codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::require_pressure_coupling_evidence;

    #[test]
    fn omitted_checkerboard_action_is_an_active_acceptance_falsifier() {
        assert!(require_pressure_coupling_evidence(0.0, 0.0, 1.0e-12).is_err());
        assert!(require_pressure_coupling_evidence(0.0, 1.0e-6, 1.0e-12).is_ok());
    }
}
