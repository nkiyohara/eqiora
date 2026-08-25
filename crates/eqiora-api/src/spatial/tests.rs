use super::*;
use eqiora_solver::ExecutionTopology;

const POISSON_2D: &str =
    include_str!("../../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");
const POISSON_1D: &str = r#"
model common_plan_poisson_1d {
  domain interval = box(0, 1);
  domain lower_end = boundary(interval, axis = 0, side = lower);
  domain upper_end = boundary(interval, axis = 0, side = upper);
  representation scalar_space = continuum;
  field potential on interval as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 1;
  relation balance continuous on interval {
    -div(grad(potential)) - source_scale = 0;
  }
  relation lower_value continuous on lower_end { trace(potential) = 0; }
  relation upper_value continuous on upper_end { trace(potential) = 0; }
}
"#;

fn document() -> ModelDocument {
    ModelDocument::compile("poisson.eqi", POISSON_2D).unwrap()
}

fn intent(method: ScalarEllipticMethod, cells: usize, workers: usize) -> ScalarEllipticIntent {
    ScalarEllipticIntent::new(
        RealizationRevision::new(7),
        method,
        NonZeroUsize::new(cells).unwrap(),
        NonZeroUsize::new(workers).unwrap(),
    )
}

#[derive(Debug, Default)]
struct RecordingScalarEllipticObserver {
    cancel_at: Option<ScalarEllipticRunProgress>,
    observed: Vec<ScalarEllipticRunProgress>,
}

impl ScalarEllipticRunObserver for RecordingScalarEllipticObserver {
    fn observe(&mut self, progress: ScalarEllipticRunProgress) -> ScalarEllipticRunDirective {
        self.observed.push(progress);
        if self.cancel_at == Some(progress) {
            ScalarEllipticRunDirective::Cancel
        } else {
            ScalarEllipticRunDirective::Continue
        }
    }
}

#[test]
fn preview_is_a_stable_content_addressed_capability_decision() {
    let document = document();
    let plan = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteElement, 16, 1),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap();
    let replay = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteElement, 16, 1),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap();

    assert_eq!(plan, replay);
    assert_eq!(plan.key().len(), 64);
    assert_eq!(plan.requirements().spatial_dimension().get(), 2);
    assert_eq!(plan.requirements().scalar_type(), ScalarType::F64);
    assert_eq!(
        plan.requirements().vector_layout(),
        VectorLayoutKind::Replicated
    );
    assert_eq!(plan.cell_count(), 256);
    assert_eq!(plan.field_value_count(), 289);
    assert_eq!(plan.artifact().digest().unwrap().to_string(), plan.key());
    assert!(plan.mesh().is_none());
    let lowered = lower_scalar_elliptic_cartesian(document.program()).unwrap();
    assert_eq!(
        plan.portable_realization().domains()[0].domain(),
        lowered.domain_id()
    );
    assert_eq!(
        plan.portable_realization().fields()[0].field(),
        lowered.field_id()
    );
    assert_eq!(
        plan.portable_realization().systems()[0].operator_properties(),
        LinearOperatorProperties::SymmetricPositiveDefinite
    );
}

#[test]
fn exact_mesh_bound_preview_is_distinct_from_legacy_lazy_preview() {
    let owner = document();
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let lowered = lower_scalar_elliptic_cartesian(owner.program()).unwrap();
    let mesh = generated_cartesian_mesh(
        lowered.bounds(),
        intent(ScalarEllipticMethod::FiniteElement, 4, 1),
    )
    .unwrap();
    assert_eq!(mesh.dimension(), 2);
    assert_eq!(mesh.mesh().axis_cell_count(0), Some(4));
    assert_eq!(mesh.mesh().axis_cell_count(1), Some(4));

    let plan = owner
        .preview_scalar_elliptic_run_on_mesh(
            intent(ScalarEllipticMethod::FiniteElement, 4, 1),
            environment,
            mesh.clone(),
        )
        .unwrap();
    assert_eq!(plan.mesh(), Some(&mesh));

    let density_errors = owner
        .preview_scalar_elliptic_run_on_mesh(
            intent(ScalarEllipticMethod::FiniteElement, 5, 1),
            environment,
            mesh.clone(),
        )
        .unwrap_err();
    assert_eq!(density_errors[0].code(), codes::INVALID_REALIZATION);
    assert_eq!(
        density_errors[0].message(),
        "the supplied scalar-elliptic Mesh does not match the spatial policy density"
    );

    let incompatible_mesh = generated_cartesian_mesh(
        &[[0.0, 2.0], [0.0, 1.0]],
        intent(ScalarEllipticMethod::FiniteElement, 4, 1),
    )
    .unwrap();
    let errors = owner
        .preview_scalar_elliptic_run_on_mesh(
            intent(ScalarEllipticMethod::FiniteElement, 4, 1),
            environment,
            incompatible_mesh,
        )
        .unwrap_err();
    assert_eq!(errors[0].code(), codes::INVALID_REALIZATION);
    assert_eq!(
        errors[0].message(),
        "the supplied Cartesian Mesh does not exactly realize the Model domain"
    );

    let foreign_dimension = generated_cartesian_mesh(
        &[[0.0, 1.0], [0.0, 1.0], [0.0, 1.0]],
        intent(ScalarEllipticMethod::FiniteElement, 4, 1),
    )
    .unwrap();
    let errors = owner
        .preview_scalar_elliptic_run_on_mesh(
            intent(ScalarEllipticMethod::FiniteElement, 4, 1),
            environment,
            foreign_dimension,
        )
        .unwrap_err();
    assert_eq!(
        errors[0].message(),
        "the supplied Cartesian Mesh does not exactly realize the Model domain"
    );

    let mut observer = UninterruptedScalarEllipticRun;
    let outcome = owner
        .run_scalar_elliptic_plan_controlled(plan.clone(), environment, &mut observer)
        .unwrap();
    let ScalarEllipticRunOutcome::Completed(result) = outcome else {
        unreachable!("the uninterrupted observer cannot cancel the run")
    };
    assert_eq!(result.plan().mesh(), plan.mesh());

    let foreign_source = POISSON_2D.replace(
        "domain square = box(0, 1, 0, 1);",
        "domain square = box(0, 2, 0, 1);",
    );
    let foreign = ModelDocument::compile("foreign-poisson.eqi", &foreign_source).unwrap();
    let foreign_plan = foreign
        .preview_scalar_elliptic_run_with_generated_mesh(
            intent(ScalarEllipticMethod::FiniteElement, 4, 1),
            environment,
        )
        .unwrap();
    let mut observer = UninterruptedScalarEllipticRun;
    let foreign_execution = foreign
        .execute_scalar_elliptic_plan_controlled(
            foreign_plan,
            environment,
            Instant::now(),
            &mut observer,
        )
        .unwrap();
    let ControlledScalarEllipticExecution::Accepted(foreign_execution) = foreign_execution else {
        unreachable!("the uninterrupted observer cannot cancel the run")
    };
    let mesh_errors = validate_scalar_elliptic_solution(
        &plan,
        &foreign_execution.solution,
        &foreign_execution.receipt,
    )
    .unwrap_err();
    assert!(
        mesh_errors[0]
            .message()
            .contains("executed scalar-elliptic Mesh differs")
    );
}

#[test]
fn method_revision_and_mesh_choices_change_realization_identity_not_model_identity() {
    let document = document();
    let fem = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteElement, 16, 1),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap();
    let fvm = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteVolume, 16, 1),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap();

    assert_eq!(fem.model_digest(), fvm.model_digest());
    assert_ne!(fem.key(), fvm.key());
    assert_eq!(fvm.field_value_count(), 256);
    assert_eq!(
        fvm.realization().discretization().method(),
        DiscretizationMethod::CellCenteredFiniteVolume
    );
}

#[test]
fn unsupported_workers_and_oversized_meshes_fail_before_allocation() {
    let document = document();
    let workers = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteElement, 16, 2),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap_err();
    assert_eq!(workers[0].code(), codes::INVALID_REALIZATION);
    assert!(workers[0].message().contains("serial adapter"));

    let oversized = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteElement, 500, 1),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap_err();
    assert_eq!(oversized[0].code(), codes::INVALID_REALIZATION);
    assert!(oversized[0].message().contains("before allocation"));
}

#[test]
fn generated_mesh_resource_admission_uses_the_actual_method_shape() {
    let one_dimensional = ModelDocument::compile("poisson-1d.eqi", POISSON_1D).unwrap();
    let two_dimensional = document();
    for (owner, cells) in [(&one_dimensional, 250_000), (&two_dimensional, 500)] {
        let fvm = owner
            .preview_scalar_elliptic_run_with_generated_mesh(
                intent(ScalarEllipticMethod::FiniteVolume, cells, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap();
        assert_eq!(fvm.cell_count(), MAX_SCALAR_ELLIPTIC_ENTITY_COUNT);
        assert_eq!(fvm.field_value_count(), MAX_SCALAR_ELLIPTIC_ENTITY_COUNT);
        assert!(fvm.mesh().is_some());

        let errors = owner
            .preview_scalar_elliptic_run_with_generated_mesh(
                intent(ScalarEllipticMethod::FiniteElement, cells, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap_err();
        assert_eq!(errors[0].code(), codes::INVALID_REALIZATION);
        assert!(errors[0].message().contains("before allocation"));
    }
}

#[test]
fn controlled_run_observes_only_the_three_exact_application_phases() {
    let document = document();
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    for method in [
        ScalarEllipticMethod::FiniteElement,
        ScalarEllipticMethod::FiniteVolume,
    ] {
        let plan = document
            .preview_scalar_elliptic_run(intent(method, 8, 1), environment)
            .unwrap();
        let mut observer = RecordingScalarEllipticObserver::default();
        let outcome = document
            .run_scalar_elliptic_plan_controlled(plan, environment, &mut observer)
            .unwrap();
        let ScalarEllipticRunOutcome::Completed(result) = outcome else {
            panic!("the recording observer cannot cancel the run");
        };

        assert_eq!(
            observer.observed,
            [
                ScalarEllipticRunProgress::PlanReplayed,
                ScalarEllipticRunProgress::SystemFinalized,
                ScalarEllipticRunProgress::SolutionAccepted,
            ]
        );
        assert_eq!(result.plan().intent().method(), method);
        assert_eq!(result.field_values().len(), result.field().value_count());
        result
            .plan()
            .validate_run_manifest(result.run_manifest())
            .unwrap();
    }
}

#[test]
fn cancellation_stops_at_each_exact_phase_without_a_partial_result() {
    let document = document();
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let phases = [
        ScalarEllipticRunProgress::PlanReplayed,
        ScalarEllipticRunProgress::SystemFinalized,
        ScalarEllipticRunProgress::SolutionAccepted,
    ];

    for (cancel_index, cancel_at) in phases.into_iter().enumerate() {
        let plan = document
            .preview_scalar_elliptic_run(
                intent(ScalarEllipticMethod::FiniteElement, 8, 1),
                environment,
            )
            .unwrap();
        let key = plan.key().to_owned();
        let mut observer = RecordingScalarEllipticObserver {
            cancel_at: Some(cancel_at),
            observed: Vec::new(),
        };
        let outcome = document
            .run_scalar_elliptic_plan_controlled(plan, environment, &mut observer)
            .unwrap();
        let ScalarEllipticRunOutcome::Cancelled(cancellation) = outcome else {
            panic!("the selected accepted phase must cancel the run");
        };

        assert_eq!(observer.observed, phases[..=cancel_index]);
        assert_eq!(cancellation.progress(), cancel_at);
        assert_eq!(cancellation.plan().key(), key);
    }
}

#[test]
fn serial_fem_and_fvm_return_bounded_fields_and_independent_evidence() {
    let document = document();
    for method in [
        ScalarEllipticMethod::FiniteElement,
        ScalarEllipticMethod::FiniteVolume,
    ] {
        let accepted = document
            .preview_scalar_elliptic_run(
                intent(method, 8, 1),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap();
        let result = document
            .run_scalar_elliptic_plan(accepted, ScalarEllipticExecutionEnvironment::host_serial())
            .unwrap();

        assert!(result.field().minimum().is_finite());
        assert!(result.field().maximum().is_finite());
        assert!(result.field().maximum() >= result.field().minimum());
        let extent = match method {
            ScalarEllipticMethod::FiniteElement => 9,
            ScalarEllipticMethod::FiniteVolume => 8,
        };
        assert_eq!(result.field().spatial_dimension(), 2);
        assert_eq!(result.field().logical_shape(), &[extent, extent]);
        assert_eq!(result.field_values().len(), result.field().value_count());
        assert!(result.field_values().iter().all(|value| value.is_finite()));
        let minimum = result
            .field_values()
            .iter()
            .copied()
            .reduce(f64::min)
            .unwrap();
        let maximum = result
            .field_values()
            .iter()
            .copied()
            .reduce(f64::max)
            .unwrap();
        assert_eq!(minimum, result.field().minimum());
        assert_eq!(maximum, result.field().maximum());
        assert!(result.balance().relative_imbalance() < 1.0e-12);
        assert!(result.solve().true_residual_norm() <= result.solve().residual_target());
        assert_eq!(
            result.solve().execution().topology(),
            ExecutionTopology::Host {
                workers: NonZeroUsize::MIN
            }
        );
        assert_eq!(result.assembly().execution(), result.solve().execution());
        assert_eq!(result.receipt().report(), result.solve());
        assert_eq!(
            result.receipt().binding().realization(),
            result.plan().portable_realization()
        );
    }
}

#[test]
fn successful_run_manifest_replays_exact_actual_host_provenance_and_linkage() {
    let document = document();
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let result = document
        .run_scalar_elliptic_plan(
            document
                .preview_scalar_elliptic_run(
                    intent(ScalarEllipticMethod::FiniteElement, 8, 1),
                    environment,
                )
                .unwrap(),
            environment,
        )
        .unwrap();

    let manifest = result.run_manifest();
    assert_eq!(manifest.model().to_string(), result.plan().model_digest());
    assert_eq!(
        manifest.realization(),
        result.plan().artifact().digest().unwrap()
    );
    assert_eq!(
        manifest.semantic_revision(),
        result.plan().artifact().semantic_revision().get()
    );
    assert!(manifest.outputs().is_empty());

    let actual = result.solve();
    let execution = manifest.execution();
    assert_eq!(execution.adapter(), actual.execution().adapter().as_str());
    assert_eq!(
        execution.adapter_version(),
        SERIAL_EXECUTION_PROVIDER.implementation_version()
    );
    assert_eq!(execution.solver_backend(), actual.backend().as_str());
    assert_eq!(
        execution.solver_backend_version(),
        REFERENCE_SOLVER_PROVIDER.implementation_version()
    );
    assert_eq!(execution.reduction(), actual.reduction());
    assert!(execution.libraries().is_empty());
    assert_eq!(
        execution.topology().unwrap(),
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        }
    );

    let bytes = manifest.canonical_json().unwrap();
    let replay = RunManifestV2::from_json(&bytes, JsonDecoderLimits::default()).unwrap();
    assert_eq!(replay.canonical_json().unwrap(), bytes);
    assert_eq!(replay.digest().unwrap(), manifest.digest().unwrap());
    replay.validate_against(result.plan().artifact()).unwrap();
    assert_eq!(
        result
            .plan()
            .replay_run_manifest(&bytes, JsonDecoderLimits::default())
            .unwrap(),
        replay
    );

    let forged_execution = ExecutionProvenanceV1::new(
        "example.forged-adapter",
        env!("CARGO_PKG_VERSION"),
        result.solve().backend().as_str(),
        env!("CARGO_PKG_VERSION"),
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        result.solve().reduction(),
    )
    .unwrap();
    let forged = RunManifestV2::new(result.plan().artifact(), forged_execution).unwrap();
    assert_eq!(
        result
            .plan()
            .validate_run_manifest(&forged)
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );

    let forged_output = manifest
        .clone()
        .with_output(eqiora_artifact::ArtifactDigest::from_hex("00".repeat(32)).unwrap());
    assert_eq!(
        result
            .plan()
            .validate_run_manifest(&forged_output)
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );

    let foreign_realization = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteVolume, 8, 1),
            environment,
        )
        .unwrap();
    assert!(
        replay
            .validate_against(foreign_realization.artifact())
            .is_err()
    );
}

#[test]
fn run_provenance_rejects_contradictory_provider_library_versions() {
    const SOLVER_LIBRARIES: &[ProviderLibrary] = &[ProviderLibrary::new("shared-runtime", "1.0.0")];
    const EXECUTION_LIBRARIES: &[ProviderLibrary] =
        &[ProviderLibrary::new("shared-runtime", "2.0.0")];
    let error = provider_execution_provenance(
        SolverProvider::new(
            eqiora_solver::BackendId::new("eqiora.test.solver"),
            "0.1.0",
            SOLVER_LIBRARIES,
        ),
        ExecutionProvider::new(
            eqiora_solver::ExecutionId::new("eqiora.test.execution"),
            "0.1.0",
            EXECUTION_LIBRARIES,
        ),
        SERIAL_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
    )
    .unwrap_err();

    assert_eq!(error.code(), codes::INVALID_ARTIFACT);
    assert!(error.message().contains("contradictory versions"));
}

#[cfg(feature = "rayon")]
#[test]
fn deployment_capacity_rejection_precedes_pool_factory() {
    use std::cell::Cell;

    let document = document();
    let environment =
        ScalarEllipticExecutionEnvironment::host_threaded(NonZeroUsize::new(2).unwrap());
    let plan = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteElement, 8, 2),
            environment,
        )
        .unwrap();
    let pool_allocations = Cell::new(0usize);
    let rejected = DeploymentBinding::bind_host(
        plan.portable_realization(),
        HostExecutorDescriptor::new(
            REFERENCE_SOLVER_PROVIDER,
            eqiora_backend_rayon::RAYON_EXECUTION_PROVIDER,
            NonZeroUsize::MIN,
            REFERENCE_LINEAR_SOLVER.capabilities(),
        ),
    )
    .and_then(|binding| {
        pool_allocations.set(pool_allocations.get() + 1);
        CpuThreadPool::from_deployment(&binding)
    })
    .unwrap_err();

    assert_eq!(rejected.code(), codes::INVALID_REALIZATION);
    assert!(rejected.message().contains("executor capacity"));
    assert_eq!(pool_allocations.get(), 0);
}

#[cfg(feature = "rayon")]
#[test]
fn threaded_execution_replays_the_same_typed_plan_and_records_workers() {
    let document = document();
    let serial = document
        .run_scalar_elliptic_plan(
            document
                .preview_scalar_elliptic_run(
                    intent(ScalarEllipticMethod::FiniteElement, 8, 1),
                    ScalarEllipticExecutionEnvironment::host_serial(),
                )
                .unwrap(),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap();
    let environment =
        ScalarEllipticExecutionEnvironment::host_threaded(NonZeroUsize::new(2).unwrap());
    let accepted = document
        .preview_scalar_elliptic_run(
            intent(ScalarEllipticMethod::FiniteElement, 8, 2),
            environment,
        )
        .unwrap();
    let result = document
        .run_scalar_elliptic_plan(accepted, environment)
        .unwrap();

    assert_eq!(
        result.solve().execution().topology(),
        ExecutionTopology::Host {
            workers: NonZeroUsize::new(2).unwrap()
        }
    );
    let manifest_execution = result.run_manifest().execution();
    assert_eq!(
        manifest_execution.adapter(),
        result.solve().execution().adapter().as_str()
    );
    assert_eq!(
        manifest_execution.solver_backend(),
        result.solve().backend().as_str()
    );
    assert_eq!(
        manifest_execution.adapter_version(),
        eqiora_backend_rayon::RAYON_ADAPTER_VERSION
    );
    assert_eq!(
        manifest_execution.solver_backend_version(),
        REFERENCE_SOLVER_PROVIDER.implementation_version()
    );
    assert_eq!(
        manifest_execution
            .libraries()
            .get("rayon")
            .map(String::as_str),
        Some(eqiora_backend_rayon::RAYON_VERSION)
    );
    assert_eq!(manifest_execution.libraries().len(), 1);
    assert_eq!(manifest_execution.reduction(), result.solve().reduction());
    assert_eq!(
        manifest_execution.topology().unwrap(),
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::new(2).unwrap(),
        }
    );
    assert_eq!(result.assembly().execution(), result.solve().execution());
    assert_eq!(
        result.solve().verification(),
        ExecutionReport::host(
            eqiora_backend_rayon::RAYON_EXECUTION,
            NonZeroUsize::new(2).unwrap()
        )
    );
    assert_eq!(
        result.receipt().acceptance_verification(),
        ExecutionReport::host_serial()
    );
    assert!(result.balance().relative_imbalance() < 1.0e-12);
    assert_eq!(result.field(), serial.field());
    assert_eq!(result.field().logical_shape(), &[9, 9]);
    assert_eq!(result.field_values(), serial.field_values());
    assert_eq!(result.balance(), serial.balance());
    assert_eq!(
        result.receipt().binding().realization().lineage(),
        serial.receipt().binding().realization().lineage()
    );
    assert_ne!(result.receipt().binding(), serial.receipt().binding());
    assert_ne!(result.solve().execution(), serial.solve().execution());
    assert_eq!(
        result.receipt().dag().steps(),
        serial.receipt().dag().steps()
    );
    assert_eq!(
        result.receipt().dag().operator(),
        serial.receipt().dag().operator()
    );
    assert_eq!(
        result.receipt().dag().solver_plan(),
        serial.receipt().dag().solver_plan()
    );
    assert_eq!(result.receipt().dimension(), serial.receipt().dimension());
}
