use eqiora_core::Diagnostic;
use eqiora_solver::{CanonicalCsrSystemView, LinearSolver, PreconditionerPolicy, SolverPlan};

use crate::binding::invalid;

pub(super) fn minimum_device_payload_bytes(
    system: &CanonicalCsrSystemView,
    plan: SolverPlan,
) -> Result<usize, Diagnostic> {
    let vector_count = match plan.algorithm() {
        LinearSolver::ConjugateGradient => 3usize,
        LinearSolver::BiConjugateGradientStabilized => 7,
        LinearSolver::MinimumResidual => 6,
        LinearSolver::SparseLu => {
            return Err(invalid(
                "device payload estimation does not implement sparse LU",
            ));
        }
    };
    let diagonal_count = usize::from(plan.preconditioner() == PreconditionerPolicy::Jacobi);
    let dimension = system.columns();
    let index_elements = system
        .row_offsets()
        .len()
        .checked_add(system.column_indices().len())
        .ok_or_else(|| invalid("device index payload size overflowed"))?;
    let scalar_elements = system
        .values()
        .len()
        .checked_add(system.right_hand_side().len())
        .and_then(|count| {
            count.checked_add(dimension.checked_mul(2 + vector_count + diagonal_count)?)
        })
        .ok_or_else(|| invalid("device scalar payload size overflowed"))?;
    index_elements
        .checked_mul(size_of::<i64>())
        .and_then(|bytes| {
            scalar_elements
                .checked_mul(size_of::<f64>())
                .and_then(|scalars| bytes.checked_add(scalars))
        })
        .ok_or_else(|| invalid("known device payload byte count overflowed"))
}
