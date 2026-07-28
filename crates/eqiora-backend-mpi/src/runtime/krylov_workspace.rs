use eqiora_core::Diagnostic;
use eqiora_distributed::DistributedLinearProblem;
use eqiora_solver::{LinearSolver, SolverPlan};

use super::{CgWorkspace, KrylovWorkspace, MinresWorkspace, invalid_realization, zeroed};

impl KrylovWorkspace {
    pub(super) fn new(
        problem: &DistributedLinearProblem<'_>,
        plan: SolverPlan,
    ) -> Result<Self, Diagnostic> {
        let dimension = problem.right_hand_side().len();
        let mut solution = zeroed(dimension, "distributed Krylov solution")?;
        if let Some(initial) = problem.initial_guess() {
            solution.copy_from_slice(initial);
        }
        match plan.algorithm() {
            LinearSolver::ConjugateGradient => Ok(Self::Cg(CgWorkspace {
                solution,
                applied: zeroed(dimension, "CG action")?,
                residual: zeroed(dimension, "CG residual")?,
                preconditioned: zeroed(dimension, "CG preconditioned residual")?,
                direction: zeroed(dimension, "CG direction")?,
                inverse_diagonal: zeroed(dimension, "Jacobi inverse diagonal")?,
            })),
            LinearSolver::MinimumResidual => Ok(Self::Minres(MinresWorkspace {
                solution,
                applied: zeroed(dimension, "MINRES action")?,
                previous_residual: zeroed(dimension, "MINRES previous residual")?,
                current_residual: zeroed(dimension, "MINRES current residual")?,
                lanczos_image: zeroed(dimension, "MINRES Lanczos image")?,
                basis: zeroed(dimension, "MINRES Lanczos basis")?,
                direction: zeroed(dimension, "MINRES direction")?,
                previous_direction: zeroed(dimension, "MINRES previous direction")?,
                older_direction: zeroed(dimension, "MINRES older direction")?,
            })),
            LinearSolver::BiConjugateGradientStabilized => Err(invalid_realization(
                "MPI distributed Krylov workspace does not implement BiCGSTAB",
            )),
            LinearSolver::SparseLu => Err(invalid_realization(
                "MPI distributed Krylov workspace does not implement sparse LU",
            )),
        }
    }
}
