use eqiora_solver::LinearSolver;

pub(super) const fn linear_solver_tag(algorithm: LinearSolver) -> u8 {
    match algorithm {
        LinearSolver::ConjugateGradient => 0,
        LinearSolver::BiConjugateGradientStabilized => 1,
        LinearSolver::MinimumResidual => 2,
        LinearSolver::SparseLu => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_report_solver_tags_are_additive_and_frozen() {
        assert_eq!(linear_solver_tag(LinearSolver::ConjugateGradient), 0);
        assert_eq!(
            linear_solver_tag(LinearSolver::BiConjugateGradientStabilized),
            1
        );
        assert_eq!(linear_solver_tag(LinearSolver::MinimumResidual), 2);
        assert_eq!(linear_solver_tag(LinearSolver::SparseLu), 3);
    }
}
