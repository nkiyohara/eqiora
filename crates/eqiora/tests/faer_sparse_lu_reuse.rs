use std::collections::BTreeSet;
use std::num::{NonZeroU16, NonZeroUsize};

use eqiora::compiler::compile;
use eqiora::diagnostic::codes;
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, ResolvedRealization, SemanticRevision, Space, SpatialDimensionSupport,
    Target, TargetCapabilities, VectorLayoutKind, resolve,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{
    BackendId, CanonicalCsrSystemView, CompleteCsrStorage, LinearOperatorProperties, LinearSolver,
    LinearSolverBackend, PreconditionerPolicy, ProviderLibrary, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolverPlan, SolverProvider,
};
use eqiora_backend_faer::{FAER_SOLVER_PROVIDER, FaerLinearSolver, FaerSparseLuReuseOwner};
use eqiora_execution::{
    AcceptedLinearExecution, AdmittedExecution, DeploymentBinding, HostExecutorDescriptor,
};
use eqiora_numerics::scalar::{
    FinalizedScalarEllipticParameterPoint, finalize_scalar_elliptic_parameter_point,
    lower_scalar_elliptic_cartesian,
};

const SOURCE: &str = r#"model faer_sparse_lu_reuse {
  domain interval = box(0, 1);
  domain lower_end = boundary(interval, axis = 0, side = lower);
  domain upper_end = boundary(interval, axis = 0, side = upper);
  representation scalar_space = continuum;

  field potential on interval as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 2;
  parameter diffusion: 1 = 1;
  parameter boundary_offset: 1 = 0;

  relation balance continuous on interval {
    -div(diffusion * grad(potential)) - source_scale = 0;
  }
  relation lower_value continuous on lower_end {
    trace(potential) - boundary_offset = 0;
  }
  relation upper_value continuous on upper_end {
    trace(potential) - boundary_offset = 0;
  }
}
"#;

const FOREIGN_SOURCE: &str = r#"model foreign_faer_sparse_lu_reuse {
  domain interval = box(0, 1);
  domain lower_end = boundary(interval, axis = 0, side = lower);
  domain upper_end = boundary(interval, axis = 0, side = upper);
  representation scalar_space = continuum;

  field potential on interval as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 2;
  parameter diffusion: 1 = 1;
  parameter boundary_offset: 1 = 0;

  relation balance continuous on interval {
    -div(diffusion * grad(potential)) - source_scale = 0;
  }
  relation lower_value continuous on lower_end {
    trace(potential) - boundary_offset = 0;
  }
  relation upper_value continuous on upper_end {
    trace(potential) - boundary_offset = 0;
  }
}
"#;

const ABSOLUTE_TOLERANCE: f64 = f64::from_bits(0x3e10_0000_0000_0000);
const SOLUTION_ERROR_CEILING: f64 = f64::from_bits(0x3df0_0000_0000_0000);
const FOREIGN_PROVIDER: SolverProvider = SolverProvider::new(
    BackendId::new("eqiora.faer.foreign-fixture"),
    "0.0.0-state-oracle",
    &[ProviderLibrary::new("faer", "0.24.4")],
);

#[derive(Clone, Copy, Debug)]
enum Point {
    P0,
    P1,
    P2,
}

struct Fixture {
    plan: SolverPlan,
    p0: FinalizedScalarEllipticParameterPoint,
    p1: FinalizedScalarEllipticParameterPoint,
    p2: FinalizedScalarEllipticParameterPoint,
    foreign_p0: FinalizedScalarEllipticParameterPoint,
}

#[test]
fn registered_public_state_machine_oracle_executes_all_falsifiers() {
    run_frozen_oracle_checker();
    cold_and_warm_executions_have_identical_acceptance_and_exact_inventory();
    full_csr_and_reuse_identities_keep_rhs_and_coefficients_separate();
    policy_identity_binds_exact_tolerances_and_normalizes_signed_zero();
    preflight_rejects_structure_provider_and_graph_mutants_without_consuming_capacity();
    singular_candidate_retains_the_last_committed_numeric_state();
    attempt_bounds_and_exhaustion_are_exact();
    every_compatible_reordering_preserves_pointwise_acceptance_and_lineage();
    owner_is_host_serial_move_only_and_has_no_ambient_storage_surface();
}

#[test]
fn policy_identity_binds_exact_tolerances_and_normalizes_signed_zero() {
    let positive_zero = sparse_lu_plan();
    let negative_zero = SolverPlan::new(
        LinearSolver::SparseLu,
        -0.0,
        ABSOLUTE_TOLERANCE,
        NonZeroUsize::MIN,
    )
    .unwrap()
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Fast);
    let alternate_tolerance = SolverPlan::new(
        LinearSolver::SparseLu,
        0.0,
        2.0 * ABSOLUTE_TOLERANCE,
        NonZeroUsize::MIN,
    )
    .unwrap()
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Fast);

    let positive = fixture_with_plan(positive_zero);
    let negative = fixture_with_plan(negative_zero);
    let alternate = fixture_with_plan(alternate_tolerance);
    let mut positive_owner = FaerSparseLuReuseOwner::new(positive_zero, nonzero(2)).unwrap();
    let mut negative_owner = FaerSparseLuReuseOwner::new(negative_zero, nonzero(2)).unwrap();
    let mut alternate_owner = FaerSparseLuReuseOwner::new(alternate_tolerance, nonzero(2)).unwrap();
    positive_owner.execute(admit(&positive.p0)).unwrap();
    negative_owner.execute(admit(&negative.p0)).unwrap();
    alternate_owner.execute(admit(&alternate.p0)).unwrap();

    assert_eq!(
        positive_owner.symbolic_reuse_identity(),
        negative_owner.symbolic_reuse_identity(),
        "policy identity must normalize both signed-zero encodings"
    );
    assert_eq!(
        positive_owner.numeric_reuse_identity(),
        negative_owner.numeric_reuse_identity()
    );
    assert_ne!(
        positive_owner.symbolic_reuse_identity(),
        alternate_owner.symbolic_reuse_identity(),
        "exact tolerance bits belong to policy identity"
    );
    assert_ne!(
        positive_owner.numeric_reuse_identity(),
        alternate_owner.numeric_reuse_identity()
    );
}

#[test]
fn cold_and_warm_executions_have_identical_acceptance_and_exact_inventory() {
    let fixture = fixture();
    let cold = [
        cold_execute(&fixture, Point::P0),
        cold_execute(&fixture, Point::P1),
        cold_execute(&fixture, Point::P2),
    ];
    let mut warm = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(64)).unwrap();

    assert_eq!(warm.plan(), fixture.plan);
    assert_eq!(warm.maximum_attempts(), nonzero(64));
    assert_counters(&warm, [0, 0, 0, 0]);
    assert_eq!(warm.symbolic_reuse_identity(), None);
    assert_eq!(warm.numeric_reuse_identity(), None);

    let accepted_p0 = warm.execute(admit(point(&fixture, Point::P0))).unwrap();
    assert_same_acceptance(&accepted_p0, &cold[0]);
    assert_point_acceptance(&accepted_p0, point(&fixture, Point::P0), 0.25);
    assert_counters(&warm, [1, 1, 1, 1]);
    let symbolic_p0 = warm.symbolic_reuse_identity().unwrap();
    let numeric_p0 = warm.numeric_reuse_identity().unwrap();

    let accepted_p1 = warm.execute(admit(point(&fixture, Point::P1))).unwrap();
    assert_same_acceptance(&accepted_p1, &cold[1]);
    assert_point_acceptance(&accepted_p1, point(&fixture, Point::P1), 0.5);
    assert_counters(&warm, [2, 2, 1, 1]);
    assert_eq!(warm.symbolic_reuse_identity(), Some(symbolic_p0));
    assert_eq!(warm.numeric_reuse_identity(), Some(numeric_p0));

    let accepted_p2 = warm.execute(admit(point(&fixture, Point::P2))).unwrap();
    assert_same_acceptance(&accepted_p2, &cold[2]);
    assert_point_acceptance(&accepted_p2, point(&fixture, Point::P2), 0.2);
    assert_counters(&warm, [3, 3, 1, 2]);
    assert_eq!(warm.symbolic_reuse_identity(), Some(symbolic_p0));
    assert_ne!(warm.numeric_reuse_identity(), Some(numeric_p0));
}

#[test]
fn full_csr_and_reuse_identities_keep_rhs_and_coefficients_separate() {
    let fixture = fixture();
    let p0 = point(&fixture, Point::P0).canonical_csr_system_view();
    let p1 = point(&fixture, Point::P1).canonical_csr_system_view();
    let p2 = point(&fixture, Point::P2).canonical_csr_system_view();

    assert_eq!(p0.row_offsets(), &[0, 1]);
    assert_eq!(p0.column_indices(), &[0]);
    assert_eq!(p0.values(), &[4.0]);
    assert_eq!(p0.right_hand_side(), &[1.0]);
    assert_eq!(p1.row_offsets(), p0.row_offsets());
    assert_eq!(p1.column_indices(), p0.column_indices());
    assert_eq!(p1.values(), p0.values());
    assert_eq!(p1.right_hand_side(), &[2.0]);
    assert_ne!(
        p0.agreement_fingerprint(),
        p1.agreement_fingerprint(),
        "the existing full-CSR identity must retain right-hand-side sensitivity"
    );
    assert_eq!(p2.row_offsets(), p0.row_offsets());
    assert_eq!(p2.column_indices(), p0.column_indices());
    assert_eq!(p2.values(), &[5.0]);
    assert_eq!(p2.right_hand_side(), &[1.0]);

    let mut owner = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(3)).unwrap();
    let p0_accepted = owner.execute(admit(point(&fixture, Point::P0))).unwrap();
    let symbolic = owner.symbolic_reuse_identity().unwrap();
    let numeric = owner.numeric_reuse_identity().unwrap();
    assert_eq!(p0_accepted.receipt().operator(), p0.agreement_fingerprint());

    let p1_accepted = owner.execute(admit(point(&fixture, Point::P1))).unwrap();
    assert_eq!(p1_accepted.receipt().operator(), p1.agreement_fingerprint());
    assert_eq!(owner.symbolic_reuse_identity(), Some(symbolic));
    assert_eq!(owner.numeric_reuse_identity(), Some(numeric));

    let p2_accepted = owner.execute(admit(point(&fixture, Point::P2))).unwrap();
    assert_eq!(p2_accepted.receipt().operator(), p2.agreement_fingerprint());
    assert_eq!(owner.symbolic_reuse_identity(), Some(symbolic));
    assert_ne!(owner.numeric_reuse_identity(), Some(numeric));
}

#[test]
fn preflight_rejects_structure_provider_and_graph_mutants_without_consuming_capacity() {
    let fixture = fixture();
    let mut owner = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(2)).unwrap();
    owner.execute(admit(point(&fixture, Point::P0))).unwrap();
    let committed = snapshot(&owner);

    let structure = Storage {
        rows: 2,
        columns: 2,
        offsets: vec![0, 2, 4],
        indices: vec![0, 1, 0, 1],
        values: vec![4.0, -2.0, -2.0, 4.0],
        right_hand_side: vec![1.0, 1.0],
    };
    let structure = CanonicalCsrSystemView::new(
        &structure,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .unwrap();
    assert_eq!(
        owner
            .execute(admit_system(
                point(&fixture, Point::P0),
                &structure,
                FAER_SOLVER_PROVIDER
            ))
            .unwrap_err()
            .code(),
        codes::INVALID_REALIZATION
    );
    assert_eq!(snapshot(&owner), committed);

    assert_eq!(
        owner
            .execute(admit_with_provider(
                point(&fixture, Point::P1),
                FOREIGN_PROVIDER,
            ))
            .unwrap_err()
            .code(),
        codes::INVALID_REALIZATION
    );
    assert_eq!(snapshot(&owner), committed);

    assert_system_bytes_equal(
        point(&fixture, Point::P0).canonical_csr_system_view(),
        fixture.foreign_p0.canonical_csr_system_view(),
    );
    assert_ne!(
        point(&fixture, Point::P0).portable_realization(),
        fixture.foreign_p0.portable_realization()
    );
    assert_eq!(
        owner
            .execute(admit(&fixture.foreign_p0))
            .unwrap_err()
            .code(),
        codes::INVALID_REALIZATION
    );
    assert_eq!(snapshot(&owner), committed);

    owner.execute(admit(point(&fixture, Point::P1))).unwrap();
    assert_counters(&owner, [2, 2, 1, 1]);
}

#[test]
fn singular_candidate_retains_the_last_committed_numeric_state() {
    let fixture = fixture();
    let mut owner = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(64)).unwrap();
    let p0 = owner.execute(admit(point(&fixture, Point::P0))).unwrap();
    let p0_identity = owner.numeric_reuse_identity();
    let p0_symbolic = owner.symbolic_reuse_identity();

    let singular = Storage {
        rows: 1,
        columns: 1,
        offsets: vec![0, 1],
        indices: vec![0],
        values: vec![0.0],
        right_hand_side: vec![1.0],
    };
    let singular = CanonicalCsrSystemView::new(
        &singular,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .unwrap();
    assert_eq!(
        owner
            .execute(admit_system(
                point(&fixture, Point::P0),
                &singular,
                FAER_SOLVER_PROVIDER
            ))
            .unwrap_err()
            .code(),
        codes::NUMERICAL_SOLVE_FAILED
    );
    assert_counters(&owner, [2, 1, 1, 1]);
    assert_eq!(owner.symbolic_reuse_identity(), p0_symbolic);
    assert_eq!(owner.numeric_reuse_identity(), p0_identity);

    let p1 = owner.execute(admit(point(&fixture, Point::P1))).unwrap();
    assert_counters(&owner, [3, 2, 1, 1]);
    assert_eq!(owner.symbolic_reuse_identity(), p0_symbolic);
    assert_eq!(owner.numeric_reuse_identity(), p0_identity);
    assert_point_acceptance(&p1, point(&fixture, Point::P1), 0.5);
    assert_ne!(p0.receipt().operator(), p1.receipt().operator());
}

#[test]
fn attempt_bounds_and_exhaustion_are_exact() {
    let fixture = fixture();
    for invalid in [1, 65, usize::MAX] {
        assert_eq!(
            FaerSparseLuReuseOwner::new(fixture.plan, nonzero(invalid))
                .unwrap_err()
                .code(),
            codes::INVALID_REALIZATION
        );
    }
    assert!(FaerSparseLuReuseOwner::new(fixture.plan, nonzero(2)).is_ok());
    assert!(FaerSparseLuReuseOwner::new(fixture.plan, nonzero(64)).is_ok());

    for unsupported in [
        fixture
            .plan
            .with_preconditioner(PreconditionerPolicy::Jacobi),
        fixture.plan.with_reduction(ReductionPolicy::Reproducible),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            0.0,
            ABSOLUTE_TOLERANCE,
            NonZeroUsize::MIN,
        )
        .unwrap(),
    ] {
        assert_eq!(
            FaerSparseLuReuseOwner::new(unsupported, nonzero(2))
                .unwrap_err()
                .code(),
            codes::INVALID_REALIZATION
        );
    }

    let mut owner = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(64)).unwrap();
    for _ in 0..64 {
        owner.execute(admit(point(&fixture, Point::P0))).unwrap();
    }
    assert_counters(&owner, [64, 64, 1, 1]);
    let committed = snapshot(&owner);
    assert_eq!(
        owner
            .execute(admit(point(&fixture, Point::P0)))
            .unwrap_err()
            .code(),
        codes::INVALID_REALIZATION
    );
    assert_eq!(snapshot(&owner), committed);
}

#[test]
fn every_compatible_reordering_preserves_pointwise_acceptance_and_lineage() {
    let fixture = fixture();
    let cold = [
        cold_execute(&fixture, Point::P0),
        cold_execute(&fixture, Point::P1),
        cold_execute(&fixture, Point::P2),
    ];
    for order in [
        [Point::P0, Point::P1, Point::P2],
        [Point::P0, Point::P2, Point::P1],
        [Point::P1, Point::P0, Point::P2],
        [Point::P1, Point::P2, Point::P0],
        [Point::P2, Point::P0, Point::P1],
        [Point::P2, Point::P1, Point::P0],
    ] {
        let mut owner = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(3)).unwrap();
        for selected in order {
            let accepted = owner.execute(admit(point(&fixture, selected))).unwrap();
            assert_same_acceptance(&accepted, &cold[point_index(selected)]);
            assert_eq!(
                accepted.receipt().binding().realization(),
                point(&fixture, selected).portable_realization()
            );
        }
        // Deliberately no cross-order factorization-count assertion: Issue #256
        // makes no phase-count order-independence claim.
    }
}

#[test]
fn owner_is_host_serial_move_only_and_has_no_ambient_storage_surface() {
    fn assert_send<T: Send>() {}
    assert_send::<FaerSparseLuReuseOwner>();

    let fixture = fixture();
    let owner = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(2)).unwrap();
    let moved_plan = std::thread::spawn(move || owner.plan()).join().unwrap();
    assert_eq!(moved_plan, fixture.plan);

    let owner_source = include_str!("../../eqiora-backend-faer/src/sparse_lu_reuse.rs");
    let factor_source = include_str!("../../eqiora-backend-faer/src/sparse_lu_factor.rs");
    let complete_source = format!("{owner_source}\n{factor_source}");
    assert!(owner_source.contains("#[derive(Debug)]"));
    assert!(
        owner_source.contains("Cell<"),
        "the frozen private Cell marker proves !Sync"
    );
    assert!(!owner_source.contains("impl Clone for FaerSparseLuReuseOwner"));
    assert!(!owner_source.contains("impl Copy for FaerSparseLuReuseOwner"));
    assert!(complete_source.contains("Par::Seq"));
    assert!(!complete_source.contains("Par::Rayon"));
    assert!(!complete_source.contains("Par::rayon"));
    for domain in [
        "eqiora.faer-sparse-lu-reuse.structure/v1\\0",
        "eqiora.faer-sparse-lu-reuse.coefficients/v1\\0",
        "eqiora.faer-sparse-lu-reuse.policy/v1\\0",
        "eqiora.faer-sparse-lu-reuse.symbolic/v1\\0",
        "eqiora.faer-sparse-lu-reuse.numeric/v1\\0",
    ] {
        assert!(
            complete_source.contains(domain),
            "missing frozen identity domain {domain}"
        );
    }
    assert!(
        complete_source.contains("to_be_bytes()"),
        "identity encodings use big-endian integer and binary64 fields"
    );
    for forbidden in [
        "OnceLock",
        "LazyLock",
        "thread_local!",
        "static mut",
        "std::env::",
        "std::fs::",
        "std::path::",
        "File::open",
        "File::create",
        "create_dir",
        "read_dir",
        "temp_dir",
        "cache_dir",
    ] {
        assert!(
            !complete_source.contains(forbidden),
            "reuse implementation must not contain ambient state primitive {forbidden}"
        );
    }

    let public_methods = owner_source
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            line.strip_prefix("pub fn ")
                .or_else(|| line.strip_prefix("pub const fn "))
                .and_then(|rest| rest.split('(').next())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_methods,
        BTreeSet::from([
            "accepted_solve_count",
            "attempted_solve_count",
            "execute",
            "maximum_attempts",
            "new",
            "numeric_factorization_count",
            "numeric_reuse_identity",
            "plan",
            "symbolic_factorization_count",
            "symbolic_reuse_identity",
        ])
    );
}

fn fixture() -> Fixture {
    fixture_with_plan(sparse_lu_plan())
}

fn fixture_with_plan(plan: SolverPlan) -> Fixture {
    let program = compile_program("faer-sparse-lu-reuse.eqi", SOURCE);
    let resolved = resolve_plan(&program, plan);
    let base = lower_scalar_elliptic_cartesian(&program).unwrap();
    assert_eq!(base.parameter_values(), &[1.0, 2.0, 0.0]);
    let parameters = base.parameter_fields().to_vec();

    let p0 = finalize_scalar_elliptic_parameter_point(base.clone(), &resolved).unwrap();
    let p1 = finalize_scalar_elliptic_parameter_point(
        base.bind_selected_parameters(&parameters, &[1.0, 4.0, 0.0])
            .unwrap(),
        &resolved,
    )
    .unwrap();
    let p2 = finalize_scalar_elliptic_parameter_point(
        base.bind_selected_parameters(&parameters, &[1.25, 2.0, 0.0])
            .unwrap(),
        &resolved,
    )
    .unwrap();

    let foreign_program = compile_program("foreign-faer-sparse-lu-reuse.eqi", FOREIGN_SOURCE);
    let foreign_resolved = resolve_plan(&foreign_program, plan);
    let foreign_p0 = finalize_scalar_elliptic_parameter_point(
        lower_scalar_elliptic_cartesian(&foreign_program).unwrap(),
        &foreign_resolved,
    )
    .unwrap();

    Fixture {
        plan,
        p0,
        p1,
        p2,
        foreign_p0,
    }
}

fn compile_program(file: &str, source: &str) -> KernelProgram {
    let mut compiled = compile(file, source).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn sparse_lu_plan() -> SolverPlan {
    SolverPlan::new(
        LinearSolver::SparseLu,
        0.0,
        ABSOLUTE_TOLERANCE,
        NonZeroUsize::MIN,
    )
    .unwrap()
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Fast)
}

fn resolve_plan(program: &KernelProgram, solver: SolverPlan) -> ResolvedRealization {
    let plan = RealizationPlan::new(
        Space::continuous_lagrange(NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: nonzero(2),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: nonzero(2),
            },
        ),
        solver,
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .unwrap();
    let capabilities = RealizationCapabilities::cartesian_product(
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
    resolve(
        &RealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(31),
            plan,
        ),
        RealizationRequirements::new(
            NonZeroUsize::MIN,
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
        &capabilities,
    )
    .unwrap()
}

fn point(fixture: &Fixture, point: Point) -> &FinalizedScalarEllipticParameterPoint {
    match point {
        Point::P0 => &fixture.p0,
        Point::P1 => &fixture.p1,
        Point::P2 => &fixture.p2,
    }
}

const fn point_index(point: Point) -> usize {
    match point {
        Point::P0 => 0,
        Point::P1 => 1,
        Point::P2 => 2,
    }
}

fn admit(point: &FinalizedScalarEllipticParameterPoint) -> AdmittedExecution<'_> {
    admit_with_provider(point, FAER_SOLVER_PROVIDER)
}

fn admit_with_provider(
    point: &FinalizedScalarEllipticParameterPoint,
    provider: SolverProvider,
) -> AdmittedExecution<'_> {
    admit_system(point, point.canonical_csr_system_view(), provider)
}

fn admit_system<'a>(
    point: &FinalizedScalarEllipticParameterPoint,
    system: &'a CanonicalCsrSystemView,
    provider: SolverProvider,
) -> AdmittedExecution<'a> {
    let binding = DeploymentBinding::bind_host(
        point.portable_realization(),
        HostExecutorDescriptor::new(
            provider,
            SERIAL_EXECUTION_PROVIDER,
            NonZeroUsize::MIN,
            FaerLinearSolver.capabilities(),
        ),
    )
    .unwrap();
    AdmittedExecution::admit_host_linear(point.portable_realization(), system, binding).unwrap()
}

fn cold_execute(fixture: &Fixture, selected: Point) -> AcceptedLinearExecution {
    let mut owner = FaerSparseLuReuseOwner::new(fixture.plan, nonzero(2)).unwrap();
    let accepted = owner.execute(admit(point(fixture, selected))).unwrap();
    assert_counters(&owner, [1, 1, 1, 1]);
    accepted
}

fn assert_point_acceptance(
    accepted: &AcceptedLinearExecution,
    point: &FinalizedScalarEllipticParameterPoint,
    expected: f64,
) {
    assert_eq!(accepted.solution().values().len(), 1);
    assert!((accepted.solution().values()[0] - expected).abs() <= SOLUTION_ERROR_CEILING);
    assert!(accepted.solution().report().true_residual_norm() <= ABSOLUTE_TOLERANCE);
    assert_eq!(
        accepted.receipt().operator(),
        point.canonical_csr_system_view().agreement_fingerprint()
    );
    assert_eq!(
        accepted.receipt().binding().realization(),
        point.portable_realization()
    );
    assert_eq!(accepted.receipt().solver_provider(), FAER_SOLVER_PROVIDER);
}

fn assert_same_acceptance(left: &AcceptedLinearExecution, right: &AcceptedLinearExecution) {
    assert_eq!(left.solution(), right.solution());
    assert_eq!(left.receipt(), right.receipt());
    assert_eq!(
        left.solution().report().true_residual_norm().to_bits(),
        right.solution().report().true_residual_norm().to_bits()
    );
    assert_eq!(
        left.receipt().binding().realization().lineage(),
        right.receipt().binding().realization().lineage()
    );
}

fn assert_counters(owner: &FaerSparseLuReuseOwner, expected: [usize; 4]) {
    assert_eq!(owner.attempted_solve_count(), expected[0]);
    assert_eq!(owner.accepted_solve_count(), expected[1]);
    assert_eq!(owner.symbolic_factorization_count(), expected[2]);
    assert_eq!(owner.numeric_factorization_count(), expected[3]);
}

fn snapshot(owner: &FaerSparseLuReuseOwner) -> ([usize; 4], Option<[u8; 32]>, Option<[u8; 32]>) {
    (
        [
            owner.attempted_solve_count(),
            owner.accepted_solve_count(),
            owner.symbolic_factorization_count(),
            owner.numeric_factorization_count(),
        ],
        owner.symbolic_reuse_identity(),
        owner.numeric_reuse_identity(),
    )
}

fn assert_system_bytes_equal(left: &CanonicalCsrSystemView, right: &CanonicalCsrSystemView) {
    assert_eq!(left.rows(), right.rows());
    assert_eq!(left.columns(), right.columns());
    assert_eq!(left.row_offsets(), right.row_offsets());
    assert_eq!(left.column_indices(), right.column_indices());
    assert_eq!(left.values(), right.values());
    assert_eq!(left.right_hand_side(), right.right_hand_side());
    assert_eq!(left.agreement_fingerprint(), right.agreement_fingerprint());
}

fn run_frozen_oracle_checker() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("eqiora crate belongs to the workspace root");
    let python = std::env::var_os("PYTHON").unwrap_or_else(|| "python3".into());
    let output = std::process::Command::new(python)
        .current_dir(repository)
        .arg("verify/numerics/faer-sparse-lu-reuse/run_case.py")
        .arg("--check")
        .output()
        .expect("the registered sparse-LU reuse case requires Python 3");
    assert!(
        output.status.success(),
        "sparse-LU reuse oracle failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("frozen test values are nonzero")
}

struct Storage {
    rows: usize,
    columns: usize,
    offsets: Vec<usize>,
    indices: Vec<usize>,
    values: Vec<f64>,
    right_hand_side: Vec<f64>,
}

impl CompleteCsrStorage for Storage {
    fn rows(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.columns
    }

    fn row_offsets(&self) -> &[usize] {
        &self.offsets
    }

    fn column_indices(&self) -> &[usize] {
        &self.indices
    }

    fn values(&self) -> &[f64] {
        &self.values
    }

    fn right_hand_side(&self) -> &[f64] {
        &self.right_hand_side
    }
}
