use eqiora_core::Diagnostic;
use eqiora_solver::{CanonicalCsrSystemView, LinearSolver, PreconditionerPolicy, SolverPlan};

use crate::runtime::solve_failed;

pub(super) fn inverse_diagonal(
    system: &CanonicalCsrSystemView,
    plan: SolverPlan,
) -> Result<Option<Vec<f64>>, Diagnostic> {
    if plan.preconditioner() == PreconditionerPolicy::Identity {
        return Ok(None);
    }
    let mut inverse = Vec::with_capacity(system.rows());
    for row in 0..system.rows() {
        let start = system.row_offsets()[row];
        let end = system.row_offsets()[row + 1];
        let columns = &system.column_indices()[start..end];
        let value = columns
            .binary_search(&row)
            .ok()
            .map_or(0.0, |offset| system.values()[start + offset]);
        let accepted = match plan.algorithm() {
            LinearSolver::ConjugateGradient => value.is_finite() && value > 0.0,
            LinearSolver::BiConjugateGradientStabilized => value.is_finite() && value != 0.0,
            LinearSolver::MinimumResidual | LinearSolver::SparseLu => false,
        };
        if !accepted {
            return Err(solve_failed(format!(
                "CUDA {:?} Jacobi preconditioning rejected diagonal entry {row}: {value:e}",
                plan.algorithm()
            )));
        }
        let reciprocal = 1.0 / value;
        if !reciprocal.is_finite() {
            return Err(solve_failed(format!(
                "CUDA Jacobi inverse diagonal overflowed at entry {row}"
            )));
        }
        inverse.push(reciprocal);
    }
    Ok(Some(inverse))
}
