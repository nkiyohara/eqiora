//! Private solver-decision plane for common root resolution.
//!
//! These functions select one exact admitted solver/provider tuple. They do
//! not recognize mathematics, choose a Formulation, admit spatial resources,
//! construct a Realization, or execute numerical work.

use super::spatial_planning::TransientSpatialDecision;
use super::*;

pub(super) fn resolve_reference_spd(
    request: CommonLinearRequest,
) -> Result<NativeLinearPolicy, Diagnostic> {
    require_method_specific(request, "reference-SPD")?;
    resolve_exact(
        request,
        LinearSolver::ConjugateGradient,
        ReductionPolicy::Reproducible,
        LinearOperatorProperties::SymmetricPositiveDefinite,
        &REFERENCE_LINEAR_SOLVER,
    )
}

pub(super) fn resolve_stokes_mini(
    request: CommonLinearRequest,
    backend: &dyn LinearSolverBackend,
) -> Result<NativeLinearPolicy, Diagnostic> {
    require_method_specific(request, "steady-Stokes MINI/P1")?;
    resolve_exact(
        request,
        LinearSolver::SparseLu,
        ReductionPolicy::Fast,
        LinearOperatorProperties::SymmetricIndefinite,
        backend,
    )
}

pub(super) fn resolve_transient_flow(
    request: CommonLinearRequest,
    spatial: TransientSpatialDecision,
    mini_backend: &dyn LinearSolverBackend,
) -> Result<NativeLinearPolicy, Diagnostic> {
    match spatial {
        TransientSpatialDecision::MiniP1 => {
            if request.objective().is_some() {
                return Err(invalid(
                    "program-controlled host-serial planning currently admits only the cell-centered General canonical-CSR transient profile",
                ));
            }
            resolve_exact(
                request,
                LinearSolver::SparseLu,
                ReductionPolicy::Fast,
                LinearOperatorProperties::General,
                mini_backend,
            )
        }
        TransientSpatialDecision::CellCentered => match request.objective() {
            None => resolve_exact(
                request,
                LinearSolver::BiConjugateGradientStabilized,
                ReductionPolicy::Reproducible,
                LinearOperatorProperties::General,
                &REFERENCE_LINEAR_SOLVER,
            ),
            Some(objective) => resolve_program_controlled(request, objective, mini_backend),
        },
    }
}

fn resolve_program_controlled(
    request: CommonLinearRequest,
    objective: SolverPlanningObjective,
    faer_backend: &dyn LinearSolverBackend,
) -> Result<NativeLinearPolicy, Diagnostic> {
    let decision = eqiora_solver::plan_host_serial_solver_v1(
        eqiora_solver::HostSerialSolverProfile::general_canonical_csr(),
        objective,
        request.relative_tolerance(),
        request.absolute_tolerance(),
        request.maximum_iterations(),
        &REFERENCE_LINEAR_SOLVER,
        faer_backend,
    )?;
    let backend: &dyn LinearSolverBackend = if decision.solver_provider()
        == REFERENCE_LINEAR_SOLVER.provider()
    {
        &REFERENCE_LINEAR_SOLVER
    } else if decision.solver_provider() == faer_backend.provider() {
        faer_backend
    } else {
        return Err(invalid(
            "host-serial planning selected a provider outside the admitted common resolver catalog",
        ));
    };
    NativeLinearPolicy::exact(decision.solver_plan(), backend)?.with_planning(&decision)
}

pub(super) fn resolve_fixed_reference_fsi(
    request: CommonLinearRequest,
) -> Result<SolverPlan, Diagnostic> {
    require_method_specific(request, "fixed-reference FSI")?;
    let plan = request.resolve(LinearSolver::MinimumResidual, ReductionPolicy::Reproducible)?;
    REFERENCE_LINEAR_SOLVER.capabilities().require_problem(
        plan,
        ScalarType::F64,
        LinearOperatorProperties::SymmetricIndefinite,
    )?;
    Ok(plan)
}

fn require_method_specific(request: CommonLinearRequest, profile: &str) -> Result<(), Diagnostic> {
    if request.objective().is_some() {
        return Err(invalid(format!(
            "program-controlled host-serial planning does not admit the {profile} profile"
        )));
    }
    Ok(())
}

fn resolve_exact(
    controls: CommonLinearRequest,
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

    fn controls() -> CommonLinearRequest {
        CommonLinearRequest::new(1.0e-8, 1.0e-10, NonZeroUsize::new(100).unwrap()).unwrap()
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
