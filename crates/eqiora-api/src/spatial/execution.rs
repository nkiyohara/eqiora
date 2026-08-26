//! Carrying out one accepted scalar-elliptic artifact.
//!
//! These are the steps between an accepted plan and a reported result:
//! selecting an executor for the requested placement, driving the solve under
//! cancellation, and recording the provenance of what actually ran.

use super::diagnostic::{capability_error, single};
use super::plan::{
    ScalarEllipticExecutionEnvironment, ScalarEllipticRunCancellation, ScalarEllipticRunDirective,
    ScalarEllipticRunObserver, ScalarEllipticRunPlan, ScalarEllipticRunProgress,
    UninterruptedScalarEllipticRun,
};
use crate::ModelDocument;
use eqiora_artifact::{
    CartesianMeshEnvelopeV1, ExecutionProvenanceV1, ExecutionTopologyV1, RunManifestV2,
};
#[cfg(feature = "rayon")]
use eqiora_backend_rayon::{CpuThreadPool, RAYON_EXECUTION_PROVIDER};
use eqiora_core::Diagnostic;
use eqiora_execution::{
    AdmittedExecution, DeploymentBinding, ExecutionReceipt, HostExecutorDescriptor,
};
#[cfg(feature = "rayon")]
use eqiora_numerics::scalar::finalize_resolved_scalar_elliptic_cartesian_with_assembly;
use eqiora_numerics::{
    scalar::AcceptedScalarEllipticParameterPoint, scalar::FinalizedScalarEllipticCartesianProblem,
    scalar::FinalizedScalarEllipticParameterPoint, scalar::ResolvedScalarEllipticCartesianSolution,
    scalar::ScalarEllipticCartesianModel, scalar::finalize_scalar_elliptic_parameter_point,
};
use eqiora_realization::{
    DiscretizationMethod, MeshKind, PortableRealizationGraph, RealizationCapabilities,
    SpatialDimensionSupport, TargetCapabilities, VectorLayoutKind,
};
use eqiora_solver::{
    CanonicalCsrSystemView, ExecutionProvider, ExecutionTopology as SolverExecutionTopology,
    LinearOperatorProperties, LinearProblem, LinearSolution, LinearSolver, LinearSolverBackend,
    PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, REFERENCE_SOLVER_PROVIDER, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
    SolverProvider,
};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

pub(crate) fn execute_bound_scalar_elliptic_point(
    plan: &ScalarEllipticRunPlan,
    model: ScalarEllipticCartesianModel,
) -> Result<(AcceptedScalarEllipticParameterPoint, ExecutionReceipt), Vec<Diagnostic>> {
    if plan.environment != ScalarEllipticExecutionEnvironment::host_serial()
        || plan.intent.workers != NonZeroUsize::MIN
    {
        return Err(single(capability_error(
            "differentiable Parameter-point execution admits the host-serial adapter only",
        )));
    }
    let binding = DeploymentBinding::bind_host(
        &plan.portable,
        host_executor(plan.environment, plan.intent.workers),
    )
    .map_err(single)?;
    let finalized =
        finalize_scalar_elliptic_parameter_point(model, &plan.resolved).map_err(single)?;
    let mut observer = UninterruptedScalarEllipticRun;
    let Some((solution, receipt)) = solve_finalized_linear_controlled(
        binding,
        &finalized,
        &REFERENCE_LINEAR_SOLVER,
        &mut observer,
    )?
    else {
        unreachable!("the uninterrupted observer cannot request cancellation")
    };
    let solution = finalized.finish(solution).map_err(single)?;
    validate_scalar_elliptic_solution(plan, solution.solution(), &receipt)?;
    Ok((solution, receipt))
}

pub(super) fn validate_scalar_elliptic_solution(
    plan: &ScalarEllipticRunPlan,
    solution: &ResolvedScalarEllipticCartesianSolution,
    receipt: &ExecutionReceipt,
) -> Result<(), Vec<Diagnostic>> {
    let (solve, value_count) = match solution {
        ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => (
            solution.solve_report(),
            solution.field().vertex_values().len(),
        ),
        ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => {
            (solution.solve_report(), solution.cell_values().len())
        }
    };
    if solve != receipt.report() {
        return Err(single(capability_error(
            "method-native solution report differs from its accepted execution receipt",
        )));
    }
    if value_count != plan.field_value_count {
        return Err(single(capability_error(format!(
            "accepted result contains {value_count} primary field values, but the plan admits {}",
            plan.field_value_count,
        ))));
    }
    let solved_mesh = match solution {
        ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => solution.field().mesh(),
        ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => solution.mesh(),
    };
    let solved_mesh = CartesianMeshEnvelopeV1::from_mesh(solved_mesh).map_err(single)?;
    if solved_mesh != plan.mesh {
        return Err(single(capability_error(
            "executed scalar-elliptic Mesh differs from the exact Mesh retained by the Plan",
        )));
    }
    Ok(())
}

#[derive(Debug)]
pub(super) enum ControlledScalarEllipticExecution {
    Accepted(Box<AcceptedScalarEllipticRun>),
    Cancelled(Box<ScalarEllipticRunCancellation>),
}

#[derive(Debug)]
pub(crate) struct AcceptedScalarEllipticRun {
    pub(crate) plan: ScalarEllipticRunPlan,
    pub(crate) elapsed: Duration,
    pub(crate) solution: ResolvedScalarEllipticCartesianSolution,
    pub(crate) receipt: ExecutionReceipt,
}

pub(super) fn scalar_elliptic_cancellation(
    plan: ScalarEllipticRunPlan,
    started: Instant,
    progress: ScalarEllipticRunProgress,
) -> ScalarEllipticRunCancellation {
    ScalarEllipticRunCancellation {
        plan,
        elapsed: started.elapsed(),
        progress,
    }
}

pub(super) fn scalar_elliptic_run_manifest(
    plan: &ScalarEllipticRunPlan,
    receipt: &ExecutionReceipt,
) -> Result<RunManifestV2, Vec<Diagnostic>> {
    let report = receipt.report();
    let execution = report.execution();
    let topology = match execution.topology() {
        SolverExecutionTopology::Host { workers } => ExecutionTopologyV1::Host { workers },
        SolverExecutionTopology::Distributed { .. } | SolverExecutionTopology::Cuda { .. } => {
            return Err(single(capability_error(
                "host scalar-elliptic execution produced a non-host topology",
            )));
        }
    };
    let provenance = provider_execution_provenance(
        report.solver_provider(),
        report.execution_provider(),
        report.verification_provider(),
        topology,
        report.reduction(),
    )
    .map_err(single)?;
    let manifest = RunManifestV2::new(&plan.artifact, provenance).map_err(single)?;
    plan.validate_run_manifest(&manifest).map_err(single)?;
    Ok(manifest)
}

pub(super) fn scalar_elliptic_execution_provenance(
    plan: &ScalarEllipticRunPlan,
) -> Result<ExecutionProvenanceV1, Diagnostic> {
    let workers = plan.intent.workers;
    let executor = host_executor(plan.environment, workers);
    provider_execution_provenance(
        executor.solver_provider(),
        executor.execution_provider(),
        executor.execution_provider(),
        ExecutionTopologyV1::Host { workers },
        plan.resolved.plan().solver().reduction(),
    )
}

pub(super) fn provider_execution_provenance(
    solver: SolverProvider,
    execution: ExecutionProvider,
    verification: ExecutionProvider,
    topology: ExecutionTopologyV1,
    reduction: ReductionPolicy,
) -> Result<ExecutionProvenanceV1, Diagnostic> {
    verification.validate()?;
    ExecutionProvenanceV1::from_provider_releases(
        solver,
        execution,
        topology,
        reduction,
        verification
            .libraries()
            .iter()
            .map(|library| (library.name(), library.version())),
    )
}

pub(super) fn scalar_elliptic_capabilities(
    environment: ScalarEllipticExecutionEnvironment,
) -> Result<RealizationCapabilities, Vec<Diagnostic>> {
    let solver = SolverCapabilities::exact([SolverCapability {
        algorithm: LinearSolver::ConjugateGradient,
        operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
        preconditioner: PreconditionerPolicy::Identity,
        reduction: ReductionPolicy::Reproducible,
        scalar_type: ScalarType::F64,
    }])
    .map_err(single)?;
    RealizationCapabilities::cartesian_product(
        [
            DiscretizationMethod::ContinuousGalerkin,
            DiscretizationMethod::CellCenteredFiniteVolume,
        ],
        [(
            MeshKind::GeneratedCartesian,
            SpatialDimensionSupport::inclusive(
                NonZeroUsize::MIN,
                NonZeroUsize::new(3).expect("three is non-zero"),
            )
            .map_err(single)?,
        )],
        [VectorLayoutKind::Replicated],
        solver,
        TargetCapabilities::none().with_host_cpu(environment.maximum_workers),
    )
    .map_err(single)
}

pub(super) fn host_executor(
    environment: ScalarEllipticExecutionEnvironment,
    workers: NonZeroUsize,
) -> HostExecutorDescriptor {
    let execution_provider = if workers == NonZeroUsize::MIN {
        SERIAL_EXECUTION_PROVIDER
    } else {
        #[cfg(feature = "rayon")]
        {
            RAYON_EXECUTION_PROVIDER
        }
        #[cfg(not(feature = "rayon"))]
        {
            // Preview rejects this branch before deployment binding when the
            // concrete threaded adapter is absent.
            SERIAL_EXECUTION_PROVIDER
        }
    };
    HostExecutorDescriptor::new(
        REFERENCE_SOLVER_PROVIDER,
        execution_provider,
        environment.maximum_workers,
        REFERENCE_LINEAR_SOLVER.capabilities(),
    )
}

pub(super) fn solve_finalized_controlled(
    binding: DeploymentBinding,
    finalized: FinalizedScalarEllipticCartesianProblem,
    backend: &dyn LinearSolverBackend,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(ResolvedScalarEllipticCartesianSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    let solved = solve_admitted_linear_controlled(
        binding,
        finalized.portable_realization(),
        finalized.canonical_csr_system_view(),
        finalized.linear_problem().map_err(single)?,
        finalized.solver_plan(),
        backend,
        observer,
    )?;
    let Some((solution, receipt)) = solved else {
        return Ok(None);
    };
    Ok(Some((finalized.finish(solution).map_err(single)?, receipt)))
}

pub(super) fn solve_finalized_linear_controlled(
    binding: DeploymentBinding,
    finalized: &FinalizedScalarEllipticParameterPoint,
    backend: &dyn LinearSolverBackend,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(LinearSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    solve_admitted_linear_controlled(
        binding,
        finalized.portable_realization(),
        finalized.canonical_csr_system_view(),
        finalized.linear_problem().map_err(single)?,
        finalized.solver_plan(),
        backend,
        observer,
    )
}

pub(super) fn solve_admitted_linear_controlled(
    binding: DeploymentBinding,
    portable: &PortableRealizationGraph,
    system: &CanonicalCsrSystemView,
    problem: LinearProblem<'_>,
    solver_plan: SolverPlan,
    backend: &dyn LinearSolverBackend,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(LinearSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    if observer.observe(ScalarEllipticRunProgress::SystemFinalized)
        == ScalarEllipticRunDirective::Cancel
    {
        return Ok(None);
    }
    let admitted =
        AdmittedExecution::admit_host_linear(portable, system, binding).map_err(single)?;
    let produced = backend.solve(&problem, solver_plan).map_err(single)?;
    let accepted = admitted.accept(produced).map_err(single)?;
    let (solution, receipt) = accepted.into_parts();
    Ok(Some((solution, receipt)))
}

#[cfg(feature = "rayon")]
pub(super) fn threaded_solve_controlled(
    document: &ModelDocument,
    plan: &ScalarEllipticRunPlan,
    binding: DeploymentBinding,
    observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(ResolvedScalarEllipticCartesianSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    let pool = CpuThreadPool::from_deployment(&binding).map_err(single)?;
    let solver = pool
        .solver(plan.resolved.plan().target(), &REFERENCE_LINEAR_SOLVER)
        .map_err(single)?;
    let assembly = pool
        .assembler(plan.resolved.plan().target())
        .map_err(single)?;
    let (_, finalized) = finalize_resolved_scalar_elliptic_cartesian_with_assembly(
        document.program(),
        &plan.resolved,
        &assembly,
    )
    .map_err(single)?;
    solve_finalized_controlled(binding, finalized, &solver, observer)
}

#[cfg(not(feature = "rayon"))]
pub(super) fn threaded_solve_controlled(
    _document: &ModelDocument,
    _plan: &ScalarEllipticRunPlan,
    _binding: DeploymentBinding,
    _observer: &mut impl ScalarEllipticRunObserver,
) -> Result<Option<(ResolvedScalarEllipticCartesianSolution, ExecutionReceipt)>, Vec<Diagnostic>> {
    Err(single(capability_error(
        "threaded scalar-elliptic execution is unavailable in this build",
    )))
}
