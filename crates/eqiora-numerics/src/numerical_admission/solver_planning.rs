//! Private solver-decision plane for common root resolution.
//!
//! These functions select one exact admitted solver/provider tuple. They do
//! not recognize mathematics, choose a Formulation, admit spatial resources,
//! construct a Realization, or execute numerical work.

use super::spatial_planning::TransientSpatialDecision;
use super::*;

pub(super) fn resolve_reference_spd(
    controls: CommonLinearControls,
) -> Result<NativeLinearPolicy, Diagnostic> {
    resolve_exact(
        controls,
        LinearSolver::ConjugateGradient,
        ReductionPolicy::Reproducible,
        LinearOperatorProperties::SymmetricPositiveDefinite,
        &REFERENCE_LINEAR_SOLVER,
    )
}

pub(super) fn resolve_stokes_mini(
    controls: CommonLinearControls,
    backend: &dyn LinearSolverBackend,
) -> Result<NativeLinearPolicy, Diagnostic> {
    resolve_exact(
        controls,
        LinearSolver::SparseLu,
        ReductionPolicy::Fast,
        LinearOperatorProperties::SymmetricIndefinite,
        backend,
    )
}

pub(super) fn resolve_transient_flow(
    controls: CommonLinearControls,
    spatial: TransientSpatialDecision,
    mini_backend: &dyn LinearSolverBackend,
) -> Result<NativeLinearPolicy, Diagnostic> {
    match spatial {
        TransientSpatialDecision::MiniP1 => resolve_exact(
            controls,
            LinearSolver::SparseLu,
            ReductionPolicy::Fast,
            LinearOperatorProperties::General,
            mini_backend,
        ),
        TransientSpatialDecision::CellCentered => resolve_exact(
            controls,
            LinearSolver::BiConjugateGradientStabilized,
            ReductionPolicy::Reproducible,
            LinearOperatorProperties::General,
            &REFERENCE_LINEAR_SOLVER,
        ),
    }
}

pub(super) fn resolve_fixed_reference_fsi(
    controls: CommonLinearControls,
) -> Result<SolverPlan, Diagnostic> {
    let plan = controls.resolve(LinearSolver::MinimumResidual, ReductionPolicy::Reproducible)?;
    REFERENCE_LINEAR_SOLVER.capabilities().require_problem(
        plan,
        ScalarType::F64,
        LinearOperatorProperties::SymmetricIndefinite,
    )?;
    Ok(plan)
}

fn resolve_exact(
    controls: CommonLinearControls,
    algorithm: LinearSolver,
    reduction: ReductionPolicy,
    properties: LinearOperatorProperties,
    backend: &dyn LinearSolverBackend,
) -> Result<NativeLinearPolicy, Diagnostic> {
    let decision = NativeLinearPolicy::exact(controls.resolve(algorithm, reduction)?, backend)?;
    decision
        .capabilities
        .require_problem(decision.solver, ScalarType::F64, properties)?;
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_solver::{
        BackendId, LinearProblem, LinearSolution, ReplicatedLinearExecution, SolverCapability,
    };

    #[derive(Debug)]
    struct ResolveOnlySparseBackend;

    impl LinearSolverBackend for ResolveOnlySparseBackend {
        fn provider(&self) -> SolverProvider {
            SolverProvider::new(BackendId::new("eqiora.test-solver-decision"), "1", &[])
        }

        fn capabilities(&self) -> SolverCapabilities {
            SolverCapabilities::exact([
                SolverCapability {
                    algorithm: LinearSolver::SparseLu,
                    operator_properties: LinearOperatorProperties::SymmetricIndefinite,
                    preconditioner: PreconditionerPolicy::Identity,
                    reduction: ReductionPolicy::Fast,
                    scalar_type: ScalarType::F64,
                },
                SolverCapability {
                    algorithm: LinearSolver::SparseLu,
                    operator_properties: LinearOperatorProperties::General,
                    preconditioner: PreconditionerPolicy::Identity,
                    reduction: ReductionPolicy::Fast,
                    scalar_type: ScalarType::F64,
                },
            ])
            .unwrap()
        }

        fn solve_with_execution(
            &self,
            _problem: &LinearProblem<'_>,
            _plan: SolverPlan,
            _execution: &dyn ReplicatedLinearExecution,
        ) -> Result<LinearSolution, Diagnostic> {
            unreachable!("solver decision tests never execute numerical work")
        }
    }

    fn controls() -> CommonLinearControls {
        CommonLinearControls::new(1.0e-8, 1.0e-10, NonZeroUsize::new(100).unwrap()).unwrap()
    }

    #[test]
    fn decisions_are_exact_and_method_specific_before_realization() {
        let scalar = resolve_reference_spd(controls()).unwrap();
        assert_eq!(scalar.solver.algorithm(), LinearSolver::ConjugateGradient);
        assert_eq!(scalar.solver.reduction(), ReductionPolicy::Reproducible);

        let stokes = resolve_stokes_mini(controls(), &ResolveOnlySparseBackend).unwrap();
        assert_eq!(stokes.solver.algorithm(), LinearSolver::SparseLu);
        assert_eq!(stokes.solver.reduction(), ReductionPolicy::Fast);

        let mini = resolve_transient_flow(
            controls(),
            TransientSpatialDecision::MiniP1,
            &ResolveOnlySparseBackend,
        )
        .unwrap();
        assert_eq!(mini.solver.algorithm(), LinearSolver::SparseLu);
        assert_eq!(mini.solver.reduction(), ReductionPolicy::Fast);

        let cell_centered = resolve_transient_flow(
            controls(),
            TransientSpatialDecision::CellCentered,
            &REFERENCE_LINEAR_SOLVER,
        )
        .unwrap();
        assert_eq!(
            cell_centered.solver.algorithm(),
            LinearSolver::BiConjugateGradientStabilized
        );
        assert_eq!(
            cell_centered.solver.reduction(),
            ReductionPolicy::Reproducible
        );

        let fsi = resolve_fixed_reference_fsi(controls()).unwrap();
        assert_eq!(fsi.algorithm(), LinearSolver::MinimumResidual);
        assert_eq!(fsi.reduction(), ReductionPolicy::Reproducible);
    }
}
