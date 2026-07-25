use std::f64::consts::PI;
use std::num::NonZeroUsize;

use eqiora::Diagnostic;
use eqiora::compiler::compile;
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::realization::{
    DefaultPolicyVersion, DiscretizationMethod, ExecutionSchedule, RealizationCapabilities,
    RealizationPlan, RealizationRequest, RealizationRequirements, SemanticRevision,
    SpatialDimensionSupport, Target, TargetCapabilities, VectorLayoutKind, default_plan_v0,
    resolve,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{
    DiagonalAvailability, LinearOperator, LinearOperatorProperties, LinearProblem, LinearSolver,
    LinearSolverBackend, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
};
use eqiora_backend_faer::{
    FAER_ADAPTER_VERSION, FAER_SOLVER_PROVIDER, FAER_VERSION, FaerLinearSolver,
};
use eqiora_execution::{AdmittedExecution, DeploymentBinding, HostExecutorDescriptor};
use eqiora_numerics::{
    scalar::ResolvedScalarEllipticCartesianSolution, scalar::ResolvedScalarEllipticSolution1d,
    scalar::finalize_resolved_scalar_elliptic_cartesian, scalar::solve_resolved_scalar_elliptic_1d,
};

const SOURCE: &str = include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");

#[test]
fn canonical_poisson_agrees_between_reference_and_faer_backends() {
    let program = compile_program();
    let requirements = RealizationRequirements::new(
        NonZeroUsize::MIN,
        ScalarType::F64,
        VectorLayoutKind::Replicated,
    );
    let semantic_revision = SemanticRevision::new(program.revision().0);

    let reference = resolve(
        &RealizationRequest::default(program.model(), semantic_revision, DefaultPolicyVersion::V0),
        requirements,
        &RealizationCapabilities::scalar_elliptic_reference(),
    )
    .unwrap();
    let (_, reference_solution) =
        solve_resolved_scalar_elliptic_1d(&program, &reference, &REFERENCE_LINEAR_SOLVER).unwrap();

    let default = default_plan_v0().unwrap();
    let faer_plan = RealizationPlan::new(
        default.space(),
        default.discretization(),
        default.solver().with_reduction(ReductionPolicy::Fast),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .unwrap();
    let faer_capabilities = RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            eqiora::realization::MeshKind::GeneratedCartesian,
            SpatialDimensionSupport::exact(NonZeroUsize::MIN),
        )],
        [VectorLayoutKind::Replicated],
        FaerLinearSolver.capabilities(),
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
    .unwrap();
    let faer = resolve(
        &RealizationRequest::explicit(
            program.model(),
            semantic_revision,
            eqiora::realization::RealizationRevision::new(1),
            faer_plan,
        ),
        requirements,
        &faer_capabilities,
    )
    .unwrap();
    let (_, finalized) = finalize_resolved_scalar_elliptic_cartesian(&program, &faer).unwrap();
    let binding = DeploymentBinding::bind_host(
        finalized.portable_realization(),
        HostExecutorDescriptor::new(
            FAER_SOLVER_PROVIDER,
            SERIAL_EXECUTION_PROVIDER,
            NonZeroUsize::MIN,
            FaerLinearSolver.capabilities(),
        ),
    )
    .unwrap();
    let admitted = AdmittedExecution::admit_host_linear(
        finalized.portable_realization(),
        finalized.canonical_csr_system_view(),
        binding,
    )
    .unwrap();
    let produced = FaerLinearSolver
        .solve(
            &finalized.linear_problem().unwrap(),
            finalized.solver_plan(),
        )
        .unwrap();
    let accepted = admitted.accept(produced).unwrap();
    let (linear_solution, receipt) = accepted.into_parts();
    let faer_solution = finalized.finish(linear_solution).unwrap();
    assert_eq!(receipt.solver_provider(), FAER_SOLVER_PROVIDER);
    assert_eq!(receipt.execution_provider(), SERIAL_EXECUTION_PROVIDER);
    assert_eq!(
        receipt.report().solver_provider(),
        receipt.binding().solver_provider()
    );
    assert_eq!(
        receipt.report().execution_provider(),
        receipt.binding().execution_provider()
    );
    assert_eq!(
        receipt.solver_provider().implementation_version(),
        FAER_ADAPTER_VERSION
    );
    assert_eq!(receipt.solver_provider().libraries().len(), 1);
    assert_eq!(receipt.solver_provider().libraries()[0].name(), "faer");
    assert_eq!(
        receipt.solver_provider().libraries()[0].version(),
        FAER_VERSION
    );

    let ResolvedScalarEllipticSolution1d::FiniteElement(reference) = reference_solution else {
        panic!("the reference plan selects finite elements");
    };
    let ResolvedScalarEllipticCartesianSolution::FiniteElement(faer) = faer_solution else {
        panic!("both plans select the same finite-element realization");
    };
    for (reference, faer) in reference
        .field()
        .values()
        .iter()
        .zip(faer.field().vertex_values())
    {
        assert!((reference - faer).abs() < 5.0e-13);
    }
    assert!(reference.residual_norm() < 1.0e-11);
    assert!(faer.solve_report().true_residual_norm() < 1.0e-11);
    let midpoint = faer.field().vertex_values()[faer.field().vertex_values().len() / 2];
    assert!(((PI * 0.5).sin() - midpoint).abs() < 4.0e-3);
    assert_eq!(
        SolverCapabilities::reference().reductions(),
        REFERENCE_LINEAR_SOLVER.capabilities().reductions()
    );
}

#[test]
fn faer_bicgstab_jacobi_and_exact_capability_boundary_are_registered() {
    let capabilities = FaerLinearSolver.capabilities();
    assert_eq!(
        capabilities.combinations(),
        &std::collections::BTreeSet::from([
            SolverCapability {
                algorithm: LinearSolver::ConjugateGradient,
                operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            },
            SolverCapability {
                algorithm: LinearSolver::ConjugateGradient,
                operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
                preconditioner: PreconditionerPolicy::Jacobi,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            },
            SolverCapability {
                algorithm: LinearSolver::BiConjugateGradientStabilized,
                operator_properties: LinearOperatorProperties::General,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            },
            SolverCapability {
                algorithm: LinearSolver::BiConjugateGradientStabilized,
                operator_properties: LinearOperatorProperties::General,
                preconditioner: PreconditionerPolicy::Jacobi,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            },
        ])
    );

    let operator = DenseOperator {
        entries: [[4.0, 1.0], [2.0, 3.0]],
    };
    let problem =
        LinearProblem::new(&operator, &[6.0, 8.0], LinearOperatorProperties::General).unwrap();
    let plan = SolverPlan::new(
        LinearSolver::BiConjugateGradientStabilized,
        1.0e-12,
        1.0e-14,
        NonZeroUsize::new(100).unwrap(),
    )
    .unwrap()
    .with_preconditioner(PreconditionerPolicy::Jacobi)
    .with_reduction(ReductionPolicy::Fast);
    let solution = FaerLinearSolver.solve(&problem, plan).unwrap();
    assert!((solution.values()[0] - 1.0).abs() < 1.0e-12);
    assert!((solution.values()[1] - 2.0).abs() < 1.0e-12);
    assert!(solution.report().true_residual_norm() <= solution.report().residual_target());

    for (unsupported, properties) in [
        (
            plan.with_preconditioner(PreconditionerPolicy::Identity),
            LinearOperatorProperties::SymmetricPositiveDefinite,
        ),
        (
            plan.with_reduction(ReductionPolicy::Reproducible),
            LinearOperatorProperties::General,
        ),
    ] {
        let error = capabilities
            .require_problem(unsupported, ScalarType::F64, properties)
            .unwrap_err();
        assert!(error.message().contains("exact"));
    }
}

#[derive(Debug)]
struct DenseOperator {
    entries: [[f64; 2]; 2],
}

impl LinearOperator for DenseOperator {
    fn rows(&self) -> usize {
        2
    }

    fn columns(&self) -> usize {
        2
    }

    fn apply(&self, input: &[f64], output: &mut [f64]) -> Result<(), Diagnostic> {
        for (row, target) in self.entries.iter().zip(output) {
            *target = row
                .iter()
                .zip(input)
                .map(|(entry, value)| entry * value)
                .sum();
        }
        Ok(())
    }

    fn diagonal(&self, output: &mut [f64]) -> Result<DiagonalAvailability, Diagnostic> {
        output.copy_from_slice(&[self.entries[0][0], self.entries[1][1]]);
        Ok(DiagonalAvailability::Available)
    }
}

fn compile_program() -> KernelProgram {
    let mut compiled =
        compile("verify/numerics/poisson-fem-fvm/models/poisson.eqi", SOURCE).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}
