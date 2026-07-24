use std::num::{NonZeroU16, NonZeroUsize};

#[cfg(feature = "threaded")]
use std::cell::Cell;

#[cfg(feature = "threaded")]
use eqiora::api::{
    ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
};
use eqiora::compiler::compile;
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::numerics::{
    ResolvedScalarEllipticCartesianSolution, ResolvedScalarEllipticSolution1d,
    lower_scalar_elliptic_1d, solve_resolved_scalar_elliptic_1d,
    solve_resolved_scalar_elliptic_1d_with_assembly, solve_resolved_scalar_elliptic_cartesian,
    solve_resolved_scalar_elliptic_cartesian_with_assembly,
};
use eqiora::realization::{
    AlgebraicBlock, AlgebraicBlockScale, DefaultPolicyVersion, Discretization,
    DiscretizationMethod, ExecutionSchedule, FieldSpaceBinding, FieldwiseRealizationPlan,
    FieldwiseRealizationRequest, FieldwiseRealizationRequirements, FieldwiseSpatialDiscretization,
    MeshArtifactReference, MeshKind, MeshPolicy, PlacementRequirementNode, PositivePhysicalScale,
    QuadraturePolicy, RealizationCapabilities, RealizationCapability, RealizationCapabilityContext,
    RealizationPlan, RealizationRequest, RealizationRequirements, RealizationRevision,
    ScheduleCapability, SemanticRevision, SingleFieldOperatorClaim, SolveRoot, Space,
    SpatialCapability, SpatialDimensionSupport, SymmetricCongruenceScaling, Target,
    TargetCapability, VectorLayoutKind, default_plan_v0, resolve, resolve_fieldwise,
};
use eqiora::sem::KernelProgram;
#[cfg(feature = "threaded")]
use eqiora::solver::{
    ConvergenceReason, ExecutionProvider, ProviderLibrary, REFERENCE_SOLVER_PROVIDER,
    SERIAL_EXECUTION_PROVIDER, SERIAL_LINEAR_EXECUTION, SolverProvider,
    accept_linear_solution_with_verifier,
};
use eqiora::solver::{
    ExecutionReport, LinearOperatorProperties, LinearSolver, PreconditionerPolicy,
    REFERENCE_LINEAR_SOLVER, ReductionPolicy, ScalarType, SolveReport, SolverCapabilities,
    SolverCapability, SolverPlan,
};
#[cfg(feature = "threaded")]
use eqiora_execution::{AdmittedExecution, DeploymentBinding, HostExecutorDescriptor};
use eqiora_fabric::{CpuThreadPool, RAYON_EXECUTION};
#[cfg(feature = "threaded")]
use eqiora_fabric::{RAYON_ADAPTER_VERSION, RAYON_EXECUTION_PROVIDER, RAYON_VERSION};
#[cfg(feature = "threaded")]
use eqiora_solver::{CanonicalCsrSystemView, CompleteCsrStorage, LinearSolverBackend};

const SOURCE: &str = include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");
const SOURCE_2D: &str =
    include_str!("../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");
const SOURCE_3D: &str =
    include_str!("../../../verify/numerics/cartesian-poisson-3d-fem-fvm/models/poisson.eqi");

#[test]
fn realization_admission_rejects_unverified_axis_recombination() {
    let dimension = NonZeroUsize::new(2).unwrap();
    let common_context = |method, mesh_kind| {
        RealizationCapabilityContext::new(
            SpatialCapability::new(method, mesh_kind, SpatialDimensionSupport::exact(dimension)),
            VectorLayoutKind::Replicated,
            TargetCapability::HostCpu {
                maximum_threads: NonZeroUsize::MIN,
            },
            ScheduleCapability::Offline,
        )
    };
    let fem_solver = SolverCapability {
        algorithm: LinearSolver::ConjugateGradient,
        operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
        preconditioner: PreconditionerPolicy::Identity,
        reduction: ReductionPolicy::Reproducible,
        scalar_type: ScalarType::F64,
    };
    let fvm_solver = SolverCapability {
        algorithm: LinearSolver::BiConjugateGradientStabilized,
        operator_properties: LinearOperatorProperties::General,
        preconditioner: PreconditionerPolicy::Jacobi,
        reduction: ReductionPolicy::Reproducible,
        scalar_type: ScalarType::F64,
    };
    let capabilities = RealizationCapabilities::exact([
        RealizationCapability::new(
            common_context(
                DiscretizationMethod::ContinuousGalerkin,
                MeshKind::ImportedAffineSimplicial,
            ),
            fem_solver,
        )
        .unwrap(),
        RealizationCapability::new(
            common_context(
                DiscretizationMethod::CellCenteredFiniteVolume,
                MeshKind::GeneratedCartesian,
            ),
            fvm_solver,
        )
        .unwrap(),
    ])
    .unwrap();
    let domain = eqiora::Id::new();
    let field = eqiora::Id::new();
    let requirements = FieldwiseRealizationRequirements::new(
        domain,
        [field],
        RealizationRequirements::new(dimension, ScalarType::F64, VectorLayoutKind::Replicated),
    )
    .unwrap();

    let fem = fieldwise_request(domain, field, FieldwisePath::Fem, fem_solver);
    let fvm = fieldwise_request(domain, field, FieldwisePath::Fvm, fvm_solver);
    assert!(
        resolve_fieldwise(&fem, requirements.clone(), &capabilities).is_ok(),
        "the explicitly admitted FEM path must resolve"
    );
    assert!(
        resolve_fieldwise(&fvm, requirements.clone(), &capabilities).is_ok(),
        "the explicitly admitted FVM path must resolve"
    );

    let recombined = fieldwise_request(domain, field, FieldwisePath::Fem, fvm_solver);
    let error = resolve_fieldwise(&recombined, requirements, &capabilities).unwrap_err();
    assert_eq!(error.code(), eqiora::diagnostic::codes::INVALID_REALIZATION);
    assert!(error.message().contains("no exact tuple"));

    let program = compile_cartesian_program(
        "verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi",
        SOURCE_2D,
    );
    let generic_plan = RealizationPlan::new(
        Space::cell_constant(),
        Discretization::new(
            DiscretizationMethod::CellCenteredFiniteVolume,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(2).unwrap(),
            },
            QuadraturePolicy::CellCentroid,
        ),
        SolverPlan::new(
            fvm_solver.algorithm,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(32).unwrap(),
        )
        .unwrap()
        .with_preconditioner(fvm_solver.preconditioner),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .unwrap();
    let resolved = resolve(
        &RealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(2),
            generic_plan,
        ),
        RealizationRequirements::new(dimension, ScalarType::F64, VectorLayoutKind::Replicated),
        &capabilities,
    )
    .expect("generic compatibility resolution retains its admitted property candidate");
    let error = resolved
        .portable_graph(SingleFieldOperatorClaim::new(
            domain,
            field,
            LinearOperatorProperties::SymmetricPositiveDefinite,
        ))
        .expect_err("an equation claim outside the retained exact tuple must fail closed");
    assert_eq!(error.code(), eqiora::diagnostic::codes::INVALID_REALIZATION);
    assert!(
        error
            .message()
            .contains("not admitted for operator properties")
    );
}

#[derive(Clone, Copy)]
enum FieldwisePath {
    Fem,
    Fvm,
}

fn reference_spd_solver_capabilities() -> SolverCapabilities {
    SolverCapabilities::exact([SolverCapability {
        algorithm: LinearSolver::ConjugateGradient,
        operator_properties: LinearOperatorProperties::SymmetricPositiveDefinite,
        preconditioner: PreconditionerPolicy::Identity,
        reduction: ReductionPolicy::Reproducible,
        scalar_type: ScalarType::F64,
    }])
    .expect("the reference Poisson solver tuple is exact")
}

fn fieldwise_request(
    domain: eqiora::Id<eqiora::kinds::Domain>,
    field: eqiora::Id<eqiora::kinds::Field>,
    path: FieldwisePath,
    solver: SolverCapability,
) -> FieldwiseRealizationRequest {
    let (space, discretization) = match path {
        FieldwisePath::Fem => (
            Space::continuous_lagrange(NonZeroU16::MIN),
            Discretization::new(
                DiscretizationMethod::ContinuousGalerkin,
                MeshPolicy::ImportedSimplicial {
                    artifact: MeshArtifactReference::from_sha256([0x6a; 32]),
                },
                QuadraturePolicy::TriangleDuffyGaussLegendre {
                    points_per_axis: NonZeroUsize::new(3).unwrap(),
                },
            ),
        ),
        FieldwisePath::Fvm => (
            Space::cell_constant(),
            Discretization::new(
                DiscretizationMethod::CellCenteredFiniteVolume,
                MeshPolicy::GeneratedUniform {
                    cells_per_axis: NonZeroUsize::new(4).unwrap(),
                },
                QuadraturePolicy::CellCentroid,
            ),
        ),
    };
    let length_scale = PositivePhysicalScale::new(eqiora::DynQuantity::new(
        1.0,
        eqiora::DimExponents {
            length: 1,
            ..eqiora::DimExponents::DIMENSIONLESS
        },
    ))
    .unwrap();
    let unit_scale = PositivePhysicalScale::new(eqiora::DynQuantity::new(
        1.0,
        eqiora::DimExponents::DIMENSIONLESS,
    ))
    .unwrap();
    let spatial = FieldwiseSpatialDiscretization::new(
        domain,
        length_scale,
        [FieldSpaceBinding::new(field, space)],
        [],
        discretization,
    )
    .unwrap();
    let scaling = SymmetricCongruenceScaling::new(
        [AlgebraicBlockScale::new(
            AlgebraicBlock::Field(field),
            unit_scale,
        )],
        unit_scale,
    )
    .unwrap();
    let plan = FieldwiseRealizationPlan::new(
        spatial,
        scaling,
        solver.operator_properties,
        SolverPlan::new(
            solver.algorithm,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap()
        .with_preconditioner(solver.preconditioner)
        .with_reduction(solver.reduction),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .unwrap();
    FieldwiseRealizationRequest::explicit(
        eqiora::ontology::OntologyId::new(),
        SemanticRevision::new(1),
        RealizationRevision::new(1),
        plan,
    )
}

#[cfg(feature = "threaded")]
#[test]
fn application_receipts_share_one_host_dag_across_serial_and_rayon() {
    let document = ModelDocument::compile(
        "verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi",
        SOURCE_2D,
    )
    .unwrap();
    let cells = NonZeroUsize::new(4).unwrap();
    let serial_environment = ScalarEllipticExecutionEnvironment::host_serial();
    let serial_plan = document
        .preview_scalar_elliptic_run(
            ScalarEllipticIntent::new(
                RealizationRevision::new(20),
                ScalarEllipticMethod::FiniteElement,
                cells,
                NonZeroUsize::MIN,
            ),
            serial_environment,
        )
        .unwrap();
    let serial = document
        .run_scalar_elliptic_plan(serial_plan, serial_environment)
        .unwrap();

    let workers = NonZeroUsize::new(4).unwrap();
    let threaded_environment = ScalarEllipticExecutionEnvironment::host_threaded(workers);
    let threaded_plan = document
        .preview_scalar_elliptic_run(
            ScalarEllipticIntent::new(
                RealizationRevision::new(21),
                ScalarEllipticMethod::FiniteElement,
                cells,
                workers,
            ),
            threaded_environment,
        )
        .unwrap();
    let threaded = document
        .run_scalar_elliptic_plan(threaded_plan, threaded_environment)
        .unwrap();

    let serial_receipt = serial.receipt();
    let threaded_receipt = threaded.receipt();
    assert_eq!(serial.field(), threaded.field());
    assert_eq!(serial.balance(), threaded.balance());
    assert_eq!(serial_receipt.operator(), threaded_receipt.operator());
    assert_eq!(serial_receipt.dimension(), threaded_receipt.dimension());
    assert_eq!(serial_receipt.output(), threaded_receipt.output());
    assert_eq!(serial_receipt.solver_plan(), threaded_receipt.solver_plan());
    assert_eq!(serial_receipt.dag().steps(), threaded_receipt.dag().steps());
    assert_ne!(serial_receipt.binding(), threaded_receipt.binding());
    assert_eq!(
        serial_receipt.solver_provider(),
        serial_receipt.report().solver_provider()
    );
    assert_eq!(
        serial_receipt.execution_provider(),
        serial_receipt.report().execution_provider()
    );
    assert_eq!(
        serial_receipt.binding().solver_provider(),
        REFERENCE_SOLVER_PROVIDER
    );
    assert_eq!(
        serial_receipt.binding().execution_provider(),
        SERIAL_EXECUTION_PROVIDER
    );
    assert_eq!(
        threaded_receipt.solver_provider(),
        threaded_receipt.report().solver_provider()
    );
    assert_eq!(
        threaded_receipt.execution_provider(),
        threaded_receipt.report().execution_provider()
    );
    assert_eq!(
        threaded_receipt.binding().solver_provider(),
        REFERENCE_SOLVER_PROVIDER
    );
    assert_eq!(
        threaded_receipt.binding().execution_provider(),
        RAYON_EXECUTION_PROVIDER
    );
    assert_eq!(
        serial_receipt.report().execution(),
        ExecutionReport::host_serial()
    );
    assert_eq!(
        threaded_receipt.report().execution(),
        ExecutionReport::host(RAYON_EXECUTION, workers)
    );
    assert_eq!(
        serial_receipt.report().verification(),
        ExecutionReport::host_serial()
    );
    assert_eq!(
        threaded_receipt.report().verification(),
        ExecutionReport::host(RAYON_EXECUTION, workers)
    );
    assert_eq!(
        serial_receipt.acceptance_verification(),
        ExecutionReport::host_serial()
    );
    assert_eq!(
        threaded_receipt.acceptance_verification(),
        ExecutionReport::host_serial()
    );
    let serial_run = serial.run_manifest().execution();
    assert_eq!(
        serial_run.adapter_version(),
        SERIAL_EXECUTION_PROVIDER.implementation_version()
    );
    assert_eq!(
        serial_run.solver_backend_version(),
        REFERENCE_SOLVER_PROVIDER.implementation_version()
    );
    assert!(serial_run.libraries().is_empty());
    let threaded_run = threaded.run_manifest().execution();
    assert_eq!(threaded_run.adapter(), RAYON_EXECUTION.as_str());
    assert_eq!(threaded_run.adapter_version(), RAYON_ADAPTER_VERSION);
    assert_eq!(
        threaded_run.solver_backend(),
        REFERENCE_SOLVER_PROVIDER.id().as_str()
    );
    assert_eq!(
        threaded_run.solver_backend_version(),
        REFERENCE_SOLVER_PROVIDER.implementation_version()
    );
    assert_eq!(
        threaded_run.libraries().get("rayon").map(String::as_str),
        Some(RAYON_VERSION)
    );
    assert_eq!(threaded_run.libraries().len(), 1);
}

#[cfg(feature = "threaded")]
#[test]
fn host_execution_admission_fails_before_pool_effects_and_rejects_substitution() {
    let document = ModelDocument::compile(
        "verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi",
        SOURCE_2D,
    )
    .unwrap();
    let workers = NonZeroUsize::new(4).unwrap();
    let environment = ScalarEllipticExecutionEnvironment::host_threaded(workers);
    let plan = document
        .preview_scalar_elliptic_run(
            ScalarEllipticIntent::new(
                RealizationRevision::new(22),
                ScalarEllipticMethod::FiniteElement,
                NonZeroUsize::new(4).unwrap(),
                workers,
            ),
            environment,
        )
        .unwrap();

    let pool_materializations = Cell::new(0usize);
    let rejected = DeploymentBinding::bind_host(
        plan.portable_realization(),
        HostExecutorDescriptor::new(
            REFERENCE_SOLVER_PROVIDER,
            RAYON_EXECUTION_PROVIDER,
            NonZeroUsize::new(2).unwrap(),
            REFERENCE_LINEAR_SOLVER.capabilities(),
        ),
    )
    .and_then(|binding| {
        pool_materializations.set(pool_materializations.get() + 1);
        CpuThreadPool::from_deployment(&binding)
    })
    .unwrap_err();
    assert!(rejected.message().contains("executor capacity"));
    assert_eq!(pool_materializations.get(), 0);

    struct TwoByTwo;
    impl CompleteCsrStorage for TwoByTwo {
        fn rows(&self) -> usize {
            2
        }
        fn columns(&self) -> usize {
            2
        }
        fn row_offsets(&self) -> &[usize] {
            &[0, 2, 4]
        }
        fn column_indices(&self) -> &[usize] {
            &[0, 1, 0, 1]
        }
        fn values(&self) -> &[f64] {
            &[2.0, -1.0, -1.0, 2.0]
        }
        fn right_hand_side(&self) -> &[f64] {
            &[1.0, 0.0]
        }
    }

    let system = CanonicalCsrSystemView::new(
        &TwoByTwo,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .unwrap();
    let binding = DeploymentBinding::bind_host(
        plan.portable_realization(),
        HostExecutorDescriptor::new(
            REFERENCE_SOLVER_PROVIDER,
            RAYON_EXECUTION_PROVIDER,
            workers,
            REFERENCE_LINEAR_SOLVER.capabilities(),
        ),
    )
    .unwrap();
    let problem = system.linear_problem().unwrap();
    let solver_plan = plan.realization().solver();
    let values = vec![2.0 / 3.0, 1.0 / 3.0];
    let candidate = |solver_provider, execution_provider| {
        accept_linear_solution_with_verifier(
            &problem,
            solver_plan,
            solver_provider,
            execution_provider,
            ExecutionReport::host(RAYON_EXECUTION, workers),
            ConvergenceReason::ResidualToleranceSatisfied,
            1,
            0.0,
            values.clone(),
            &SERIAL_LINEAR_EXECUTION,
        )
        .unwrap()
    };

    let substituted_solver =
        SolverProvider::new(REFERENCE_SOLVER_PROVIDER.id(), "0.1.0-substituted", &[]);
    let admitted =
        AdmittedExecution::admit_host_linear(plan.portable_realization(), &system, binding.clone())
            .unwrap();
    let rejected = admitted
        .accept(candidate(substituted_solver, RAYON_EXECUTION_PROVIDER))
        .unwrap_err();
    assert!(
        rejected
            .message()
            .contains("provider provenance contradicts")
    );

    const SUBSTITUTED_RAYON: &[ProviderLibrary] =
        &[ProviderLibrary::new("rayon", "0.0.0-substituted")];
    let substituted_execution = ExecutionProvider::new(
        RAYON_EXECUTION_PROVIDER.id(),
        RAYON_EXECUTION_PROVIDER.implementation_version(),
        SUBSTITUTED_RAYON,
    );
    let admitted =
        AdmittedExecution::admit_host_linear(plan.portable_realization(), &system, binding)
            .unwrap();
    let rejected = admitted
        .accept(candidate(REFERENCE_SOLVER_PROVIDER, substituted_execution))
        .unwrap_err();
    assert!(
        rejected
            .message()
            .contains("provider provenance contradicts")
    );
}

#[test]
fn canonical_poisson_has_worker_independent_reproducible_cpu_evidence() {
    let program = compile_program();
    let requirements = RealizationRequirements::new(
        NonZeroUsize::MIN,
        ScalarType::F64,
        VectorLayoutKind::Replicated,
    );
    let semantic_revision = SemanticRevision::new(program.revision().0);
    let serial = resolve(
        &RealizationRequest::default(program.model(), semantic_revision, DefaultPolicyVersion::V0),
        requirements,
        &RealizationCapabilities::scalar_elliptic_reference(),
    )
    .unwrap();
    let lowered = lower_scalar_elliptic_1d(&program).unwrap();
    let claim = SingleFieldOperatorClaim::new(
        lowered.domain_id(),
        lowered.field_id(),
        LinearOperatorProperties::SymmetricPositiveDefinite,
    );
    let serial_graph = serial.portable_graph(claim).unwrap();
    assert!(matches!(serial_graph.root(), SolveRoot::Linear(_)));
    assert_eq!(serial_graph.domains()[0].domain(), lowered.domain_id());
    assert_eq!(serial_graph.fields()[0].field(), lowered.field_id());
    let (_, serial_solution) =
        solve_resolved_scalar_elliptic_1d(&program, &serial, &REFERENCE_LINEAR_SOLVER).unwrap();

    let workers = NonZeroUsize::new(4).unwrap();
    let pool = CpuThreadPool::new(workers).unwrap();
    let base = default_plan_v0().unwrap();
    let threaded_plan = RealizationPlan::new(
        base.space(),
        base.discretization(),
        base.solver(),
        Target::HostCpu { threads: workers },
        ExecutionSchedule::Offline,
    )
    .unwrap();
    let capabilities = RealizationCapabilities::cartesian_product(
        [
            DiscretizationMethod::ContinuousGalerkin,
            DiscretizationMethod::CellCenteredFiniteVolume,
        ],
        [(
            eqiora::realization::MeshKind::GeneratedCartesian,
            SpatialDimensionSupport::exact(NonZeroUsize::MIN),
        )],
        [VectorLayoutKind::Replicated],
        reference_spd_solver_capabilities(),
        pool.target_capabilities(),
    )
    .unwrap();
    let threaded = resolve(
        &RealizationRequest::explicit(
            program.model(),
            semantic_revision,
            RealizationRevision::new(1),
            threaded_plan,
        ),
        requirements,
        &capabilities,
    )
    .unwrap();
    let threaded_graph = threaded.portable_graph(claim).unwrap();
    assert_eq!(threaded_graph.domains(), serial_graph.domains());
    assert_eq!(threaded_graph.fields(), serial_graph.fields());
    assert_eq!(
        threaded_graph.placements(),
        [PlacementRequirementNode::HostWorkers {
            workers_per_partition: workers,
        }]
    );
    let threaded_backend = pool
        .solver(threaded.plan().target(), &REFERENCE_LINEAR_SOLVER)
        .unwrap();
    let threaded_assembly = pool.assembler(threaded.plan().target()).unwrap();
    let (_, threaded_solution) = solve_resolved_scalar_elliptic_1d_with_assembly(
        &program,
        &threaded,
        &threaded_assembly,
        &threaded_backend,
    )
    .unwrap();

    let (
        ResolvedScalarEllipticSolution1d::FiniteElement(serial),
        ResolvedScalarEllipticSolution1d::FiniteElement(threaded),
    ) = (serial_solution, threaded_solution)
    else {
        panic!("the default plan selects the P1 finite-element realization");
    };
    assert_eq!(threaded.field().values(), serial.field().values());
    assert_eq!(threaded.cell_gradients(), serial.cell_gradients());
    assert_eq!(threaded.endpoint_reactions(), serial.endpoint_reactions());
    assert_eq!(threaded.residual_norm(), serial.residual_norm());
    assert_eq!(
        serial.assembly_report().execution(),
        ExecutionReport::host_serial()
    );
    assert_eq!(
        threaded.assembly_report().execution(),
        ExecutionReport::host(RAYON_EXECUTION, workers)
    );
    assert_eq!(
        threaded.assembly_report().packet_count(),
        serial.assembly_report().packet_count()
    );
    assert_eq!(
        threaded.assembly_report().target_count(),
        serial.assembly_report().target_count()
    );
    assert_eq!(
        serial.solve_report().execution(),
        ExecutionReport::host_serial()
    );
    assert_eq!(
        threaded.solve_report().execution(),
        ExecutionReport::host(RAYON_EXECUTION, workers)
    );
    assert_eq!(
        threaded.solve_report().backend(),
        serial.solve_report().backend()
    );
    assert_eq!(
        threaded.solve_report().orientation(),
        serial.solve_report().orientation()
    );
    assert_eq!(
        threaded.solve_report().algorithm(),
        serial.solve_report().algorithm()
    );
    assert_eq!(
        threaded.solve_report().preconditioner(),
        serial.solve_report().preconditioner()
    );
    assert_eq!(
        threaded.solve_report().reduction(),
        serial.solve_report().reduction()
    );
    assert_eq!(
        threaded.solve_report().reason(),
        serial.solve_report().reason()
    );
    assert_eq!(
        threaded.solve_report().completed_iterations(),
        serial.solve_report().completed_iterations()
    );
    assert_eq!(
        threaded.solve_report().initial_residual_norm(),
        serial.solve_report().initial_residual_norm()
    );
    assert_eq!(
        threaded.solve_report().reported_residual_norm(),
        serial.solve_report().reported_residual_norm()
    );
    assert_eq!(
        threaded.solve_report().true_residual_norm(),
        serial.solve_report().true_residual_norm()
    );
    assert_eq!(
        threaded.solve_report().residual_target(),
        serial.solve_report().residual_target()
    );

    let unsupported_workers = NonZeroUsize::new(5).unwrap();
    let unsupported_plan = RealizationPlan::new(
        base.space(),
        base.discretization(),
        base.solver(),
        Target::HostCpu {
            threads: unsupported_workers,
        },
        ExecutionSchedule::Offline,
    )
    .unwrap();
    assert!(
        resolve(
            &RealizationRequest::explicit(
                program.model(),
                semantic_revision,
                RealizationRevision::new(2),
                unsupported_plan,
            ),
            requirements,
            &capabilities,
        )
        .is_err()
    );
}

#[test]
fn cartesian_fem_and_fvm_have_exact_worker_independent_2d_and_3d_evidence() {
    let workers = NonZeroUsize::new(4).unwrap();
    let pool = CpuThreadPool::new(workers).unwrap();

    for (file, source, dimension, cells) in [
        (
            "verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi",
            SOURCE_2D,
            2_usize,
            4_usize,
        ),
        (
            "verify/numerics/cartesian-poisson-3d-fem-fvm/models/poisson.eqi",
            SOURCE_3D,
            3_usize,
            3_usize,
        ),
    ] {
        let program = compile_cartesian_program(file, source);
        let dimension = NonZeroUsize::new(dimension).unwrap();
        let requirements =
            RealizationRequirements::new(dimension, ScalarType::F64, VectorLayoutKind::Replicated);
        let semantic_revision = SemanticRevision::new(program.revision().0);
        let threaded_capabilities = RealizationCapabilities::cartesian_product(
            [
                DiscretizationMethod::ContinuousGalerkin,
                DiscretizationMethod::CellCenteredFiniteVolume,
            ],
            [(
                eqiora::realization::MeshKind::GeneratedCartesian,
                SpatialDimensionSupport::exact(dimension),
            )],
            [VectorLayoutKind::Replicated],
            reference_spd_solver_capabilities(),
            pool.target_capabilities(),
        )
        .unwrap();

        for (revision, method) in [
            (0_u64, DiscretizationMethod::ContinuousGalerkin),
            (1_u64, DiscretizationMethod::CellCenteredFiniteVolume),
        ] {
            let serial = resolve(
                &RealizationRequest::explicit(
                    program.model(),
                    semantic_revision,
                    RealizationRevision::new(revision),
                    cartesian_plan(cells, dimension.get(), method, NonZeroUsize::MIN),
                ),
                requirements,
                &RealizationCapabilities::scalar_elliptic_reference(),
            )
            .unwrap();
            let threaded = resolve(
                &RealizationRequest::explicit(
                    program.model(),
                    semantic_revision,
                    RealizationRevision::new(revision + 2),
                    cartesian_plan(cells, dimension.get(), method, workers),
                ),
                requirements,
                &threaded_capabilities,
            )
            .unwrap();
            let (_, serial_solution) = solve_resolved_scalar_elliptic_cartesian(
                &program,
                &serial,
                &REFERENCE_LINEAR_SOLVER,
            )
            .unwrap();
            let threaded_solver = pool
                .solver(threaded.plan().target(), &REFERENCE_LINEAR_SOLVER)
                .unwrap();
            let threaded_assembly = pool.assembler(threaded.plan().target()).unwrap();
            let (_, threaded_solution) = solve_resolved_scalar_elliptic_cartesian_with_assembly(
                &program,
                &threaded,
                &threaded_assembly,
                &threaded_solver,
            )
            .unwrap();

            let cell_count = cells.pow(u32::try_from(dimension.get()).unwrap());
            match (serial_solution, threaded_solution) {
                (
                    ResolvedScalarEllipticCartesianSolution::FiniteElement(serial),
                    ResolvedScalarEllipticCartesianSolution::FiniteElement(threaded),
                ) => {
                    assert_eq!(threaded.field(), serial.field());
                    assert_eq!(threaded.algebraic_values(), serial.algebraic_values());
                    assert_eq!(
                        threaded.boundary_reaction_sum(),
                        serial.boundary_reaction_sum()
                    );
                    assert_eq!(threaded.integrated_source(), serial.integrated_source());
                    assert_assembly_evidence(
                        serial.assembly_report(),
                        threaded.assembly_report(),
                        workers,
                        cell_count,
                        2,
                    );
                    assert_solve_evidence(serial.solve_report(), threaded.solve_report(), workers);
                }
                (
                    ResolvedScalarEllipticCartesianSolution::FiniteVolume(serial),
                    ResolvedScalarEllipticCartesianSolution::FiniteVolume(threaded),
                ) => {
                    assert_eq!(threaded.mesh(), serial.mesh());
                    assert_eq!(threaded.cell_centers(), serial.cell_centers());
                    assert_eq!(threaded.cell_values(), serial.cell_values());
                    assert_eq!(threaded.reconstruction(), serial.reconstruction());
                    assert_eq!(threaded.boundary_flux_sum(), serial.boundary_flux_sum());
                    assert_eq!(threaded.integrated_source(), serial.integrated_source());
                    let facet_count = dimension
                        .get()
                        .checked_mul(cells + 1)
                        .and_then(|count| {
                            count
                                .checked_mul(cells.pow(u32::try_from(dimension.get() - 1).unwrap()))
                        })
                        .unwrap();
                    assert_assembly_evidence(
                        serial.assembly_report(),
                        threaded.assembly_report(),
                        workers,
                        cell_count + facet_count,
                        1,
                    );
                    assert_solve_evidence(serial.solve_report(), threaded.solve_report(), workers);
                }
                _ => panic!("serial and threaded plans selected different methods"),
            }
        }
    }
}

fn cartesian_plan(
    cells: usize,
    dimension: usize,
    method: DiscretizationMethod,
    threads: NonZeroUsize,
) -> RealizationPlan {
    let (space, quadrature) = match method {
        DiscretizationMethod::ContinuousGalerkin => (
            Space::continuous_lagrange(NonZeroU16::MIN),
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).unwrap(),
            },
        ),
        DiscretizationMethod::CellCenteredFiniteVolume => {
            (Space::cell_constant(), QuadraturePolicy::CellCentroid)
        }
    };
    let unknown_scale = cells.pow(u32::try_from(dimension).unwrap());
    RealizationPlan::new(
        space,
        Discretization::new(
            method,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(cells).unwrap(),
            },
            quadrature,
        ),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(unknown_scale * 8).unwrap(),
        )
        .unwrap(),
        Target::HostCpu { threads },
        ExecutionSchedule::Offline,
    )
    .unwrap()
}

fn assert_assembly_evidence(
    serial: &eqiora::assembly::AssemblyReport,
    threaded: &eqiora::assembly::AssemblyReport,
    workers: NonZeroUsize,
    packet_count: usize,
    target_count: usize,
) {
    assert_eq!(serial.execution(), ExecutionReport::host_serial());
    assert_eq!(
        threaded.execution(),
        ExecutionReport::host(RAYON_EXECUTION, workers)
    );
    assert_eq!(serial.packet_count(), packet_count);
    assert_eq!(threaded.packet_count(), packet_count);
    assert_eq!(serial.target_count(), target_count);
    assert_eq!(threaded.target_count(), target_count);
}

fn assert_solve_evidence(serial: &SolveReport, threaded: &SolveReport, workers: NonZeroUsize) {
    assert_eq!(serial.execution(), ExecutionReport::host_serial());
    assert_eq!(
        threaded.execution(),
        ExecutionReport::host(RAYON_EXECUTION, workers)
    );
    assert_eq!(threaded.backend(), serial.backend());
    assert_eq!(threaded.orientation(), serial.orientation());
    assert_eq!(threaded.algorithm(), serial.algorithm());
    assert_eq!(threaded.preconditioner(), serial.preconditioner());
    assert_eq!(threaded.reduction(), serial.reduction());
    assert_eq!(threaded.reason(), serial.reason());
    assert_eq!(
        threaded.completed_iterations(),
        serial.completed_iterations()
    );
    assert_eq!(
        threaded.initial_residual_norm(),
        serial.initial_residual_norm()
    );
    assert_eq!(
        threaded.reported_residual_norm(),
        serial.reported_residual_norm()
    );
    assert_eq!(threaded.true_residual_norm(), serial.true_residual_norm());
    assert_eq!(threaded.residual_target(), serial.residual_target());
}

fn compile_program() -> KernelProgram {
    let mut compiled =
        compile("verify/numerics/poisson-fem-fvm/models/poisson.eqi", SOURCE).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn compile_cartesian_program(file: &str, source: &str) -> KernelProgram {
    let mut compiled = compile(file, source).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}
