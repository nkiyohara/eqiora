//! Capability-resolved scalar-elliptic application workflow.
//!
//! The values in this module are application contracts, not Studio DTOs. A
//! client proposes one independently revisioned Realization, receives a
//! content-addressed artifact after complete capability validation, and may
//! execute only that exact accepted artifact.
mod diagnostic;
mod error_metric;
mod execution;
mod field;
mod plan;

pub(crate) use execution::execute_bound_scalar_elliptic_point;
pub use field::{
    CartesianFieldOrder, CartesianScalarFieldProjection, ScalarEllipticBalanceEvidence,
    ScalarEllipticRunResult, ScalarFieldLocation, ScalarFieldSummary,
};
pub use plan::{
    MAX_SCALAR_ELLIPTIC_ENTITY_COUNT, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent,
    ScalarEllipticMesh, ScalarEllipticMethod, ScalarEllipticRunCancellation,
    ScalarEllipticRunDirective, ScalarEllipticRunObserver, ScalarEllipticRunOutcome,
    ScalarEllipticRunPlan, ScalarEllipticRunProgress,
};

use diagnostic::{capability_error, single};
use execution::{
    AcceptedScalarEllipticRun, ControlledScalarEllipticExecution, host_executor,
    scalar_elliptic_cancellation, scalar_elliptic_capabilities, scalar_elliptic_run_manifest,
    solve_finalized_controlled, threaded_solve_controlled, validate_scalar_elliptic_solution,
};
use field::{scalar_field_projection, summarize};
use plan::{UninterruptedScalarEllipticRun, resource_shape};
use std::num::NonZeroUsize;
use std::time::Instant;

#[cfg(test)]
use eqiora_artifact::{
    ExecutionProvenanceV1, ExecutionTopologyV1, JsonDecoderLimits, RunManifestV2,
};
use eqiora_artifact::{LayoutArtifacts, RealizationEnvelopeV1};
#[cfg(all(test, feature = "rayon"))]
use eqiora_backend_rayon::CpuThreadPool;
use eqiora_core::Diagnostic;
#[cfg(test)]
use eqiora_core::diagnostic::codes;
use eqiora_execution::DeploymentBinding;
#[cfg(all(test, feature = "rayon"))]
use eqiora_execution::HostExecutorDescriptor;
use eqiora_numerics::{
    scalar::finalize_resolved_scalar_elliptic_cartesian, scalar::lower_scalar_elliptic_cartesian,
};
#[cfg(test)]
use eqiora_realization::DiscretizationMethod;
#[cfg(test)]
use eqiora_realization::RealizationRevision;
use eqiora_realization::{
    Discretization, ExecutionSchedule, MeshPolicy, RealizationPlan, RealizationRequest,
    RealizationRequirements, SemanticRevision, SingleFieldOperatorClaim, Target, VectorLayoutKind,
    resolve,
};
#[cfg(all(test, feature = "rayon"))]
use eqiora_solver::ExecutionReport;
#[cfg(all(test, feature = "rayon"))]
use eqiora_solver::LinearSolverBackend;
#[cfg(test)]
use eqiora_solver::ProviderLibrary;
#[cfg(test)]
use eqiora_solver::{
    ExecutionProvider, REFERENCE_SOLVER_PROVIDER, ReductionPolicy, SERIAL_EXECUTION_PROVIDER,
    SolverProvider,
};
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, REFERENCE_LINEAR_SOLVER, ScalarType, SolverPlan,
};
#[cfg(test)]
use execution::provider_execution_provenance;

use crate::ModelDocument;

impl ModelDocument {
    /// Resolve one explicit scalar-elliptic Realization without allocating its
    /// mesh, matrix, worker pool, or result buffers.
    ///
    /// # Errors
    /// Returns one structured lowering, resource, artifact, or capability
    /// diagnostic. Unsupported plans never fall back to a default.
    pub fn preview_scalar_elliptic_run(
        &self,
        intent: ScalarEllipticIntent,
        environment: ScalarEllipticExecutionEnvironment,
    ) -> Result<ScalarEllipticRunPlan, Vec<Diagnostic>> {
        self.preview_scalar_elliptic_run_inner(intent, environment, None)
    }

    /// Resolve one explicit scalar-elliptic Realization and materialize its
    /// exact generated Cartesian Mesh from the currently admitted Model domain.
    ///
    /// This transitional scalar seam does not move geometry authority out of
    /// the Model. It only separates the caller's typed density request from the
    /// effective Mesh owned by the resulting Plan.
    ///
    /// # Errors
    /// Returns one structured lowering, resource, Mesh, artifact, or capability
    /// diagnostic before publishing a partial Plan.
    pub fn preview_scalar_elliptic_run_with_generated_mesh(
        &self,
        intent: ScalarEllipticIntent,
        environment: ScalarEllipticExecutionEnvironment,
    ) -> Result<ScalarEllipticRunPlan, Vec<Diagnostic>> {
        let lowered = lower_scalar_elliptic_cartesian(self.program()).map_err(single)?;
        let mesh = ScalarEllipticMesh::uniform(lowered.bounds(), intent.cells_per_axis)?;
        self.preview_scalar_elliptic_run_on_mesh(intent, environment, mesh)
    }

    /// Resolve one explicit scalar-elliptic Realization against an exact
    /// effective Mesh without silently regenerating or substituting it.
    ///
    /// # Errors
    /// Rejects a foreign Model binding, mismatched mesh policy, or unsupported
    /// method/provider combination before Plan publication.
    pub(crate) fn preview_scalar_elliptic_run_on_mesh(
        &self,
        intent: ScalarEllipticIntent,
        environment: ScalarEllipticExecutionEnvironment,
        mesh: ScalarEllipticMesh,
    ) -> Result<ScalarEllipticRunPlan, Vec<Diagnostic>> {
        if mesh.cells_per_axis != intent.cells_per_axis {
            return Err(single(capability_error(
                "the supplied scalar-elliptic Mesh does not match the spatial policy density",
            )));
        }
        let lowered = lower_scalar_elliptic_cartesian(self.program()).map_err(single)?;
        let expected = ScalarEllipticMesh::uniform(lowered.bounds(), intent.cells_per_axis)?;
        if mesh != expected {
            return Err(single(capability_error(
                "the supplied Cartesian Mesh does not exactly realize the Model domain",
            )));
        }
        self.preview_scalar_elliptic_run_inner(intent, environment, Some(mesh))
    }

    fn preview_scalar_elliptic_run_inner(
        &self,
        intent: ScalarEllipticIntent,
        environment: ScalarEllipticExecutionEnvironment,
        mesh: Option<ScalarEllipticMesh>,
    ) -> Result<ScalarEllipticRunPlan, Vec<Diagnostic>> {
        let model_reference = self.artifact_reference().map_err(single)?;
        let model_digest = self.digest().map_err(single)?;
        if !environment.supports(intent.workers) {
            return Err(single(capability_error(format!(
                "host execution admits at most {} worker(s){}; {} were requested",
                environment.maximum_workers,
                if environment.threaded {
                    " through a run-owned threaded adapter"
                } else {
                    " through the serial adapter"
                },
                intent.workers,
            ))));
        }

        let model = lower_scalar_elliptic_cartesian(self.program()).map_err(single)?;
        let dimension = NonZeroUsize::new(model.dimension()).ok_or_else(|| {
            single(capability_error(
                "scalar-elliptic lowering produced a zero spatial dimension",
            ))
        })?;
        let (cell_count, field_value_count) = resource_shape(intent, dimension)?;
        let field_projection =
            scalar_field_projection(self, &model, intent, field_value_count).map_err(single)?;
        let solver = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(10_000).expect("10,000 is non-zero"),
        )
        .map_err(single)?;
        let plan = RealizationPlan::new(
            intent.method.space(),
            Discretization::new(
                intent.method.discretization(),
                MeshPolicy::GeneratedUniform {
                    cells_per_axis: intent.cells_per_axis,
                },
                intent.method.quadrature(),
            ),
            solver,
            Target::HostCpu {
                threads: intent.workers,
            },
            ExecutionSchedule::Offline,
        )
        .map_err(single)?;
        let requirements =
            RealizationRequirements::new(dimension, ScalarType::F64, VectorLayoutKind::Replicated);
        let capabilities = scalar_elliptic_capabilities(environment)?;
        let resolved = resolve(
            &RealizationRequest::explicit(
                self.program().model(),
                SemanticRevision::new(self.program().revision().0),
                intent.realization_revision,
                plan,
            ),
            requirements,
            &capabilities,
        )
        .map_err(single)?;
        let portable = resolved
            .portable_graph(SingleFieldOperatorClaim::new(
                model.domain_id(),
                model.field_id(),
                LinearOperatorProperties::SymmetricPositiveDefinite,
            ))
            .map_err(single)?;
        let artifact = RealizationEnvelopeV1::from_resolved(
            &model_reference,
            &resolved,
            LayoutArtifacts::Replicated,
        )
        .map_err(single)?;
        let key = artifact.digest().map_err(single)?.to_string();
        Ok(ScalarEllipticRunPlan {
            model_digest,
            intent,
            environment,
            resolved,
            portable,
            artifact,
            key,
            cell_count,
            field_value_count,
            field_projection,
            mesh,
        })
    }

    /// Replay and execute one exact capability-admitted scalar-elliptic plan.
    ///
    /// Capability and resource checks run again before the first numerical
    /// allocation. Complete primary Field values cross the data plane only
    /// after numerical and continuous acceptance.
    ///
    /// # Errors
    /// Returns a structured diagnostic when replay, allocation, assembly,
    /// solve, or independent acceptance fails.
    pub fn run_scalar_elliptic_plan(
        &self,
        accepted: ScalarEllipticRunPlan,
        environment: ScalarEllipticExecutionEnvironment,
    ) -> Result<ScalarEllipticRunResult, Vec<Diagnostic>> {
        let mut observer = UninterruptedScalarEllipticRun;
        match self.run_scalar_elliptic_plan_controlled(accepted, environment, &mut observer)? {
            ScalarEllipticRunOutcome::Completed(result) => Ok(*result),
            ScalarEllipticRunOutcome::Cancelled(_) => {
                unreachable!("the uninterrupted observer cannot request cancellation")
            }
        }
    }

    /// Execute one exact plan while observing only fully accepted application
    /// phases.
    ///
    /// Cancellation is a typed terminal outcome. The linear solve is one
    /// atomic interval between `SystemFinalized` and `SolutionAccepted`; a
    /// request made during that interval is observed only after the solution
    /// has passed independent acceptance. No Python or client callback runs
    /// inside assembly or the solver.
    ///
    /// # Errors
    /// Returns the same structured diagnostics as
    /// [`Self::run_scalar_elliptic_plan`].
    pub fn run_scalar_elliptic_plan_controlled(
        &self,
        accepted: ScalarEllipticRunPlan,
        environment: ScalarEllipticExecutionEnvironment,
        observer: &mut impl ScalarEllipticRunObserver,
    ) -> Result<ScalarEllipticRunOutcome, Vec<Diagnostic>> {
        let controlled_started = Instant::now();
        let accepted = match self.execute_scalar_elliptic_plan_controlled(
            accepted,
            environment,
            controlled_started,
            observer,
        )? {
            ControlledScalarEllipticExecution::Accepted(accepted) => *accepted,
            ControlledScalarEllipticExecution::Cancelled(cancellation) => {
                return Ok(ScalarEllipticRunOutcome::Cancelled(cancellation));
            }
        };
        let (field, balance, assembly, solve) = summarize(&accepted.solution)?;
        if !accepted.plan.field_projection.matches_summary(field) {
            return Err(single(capability_error(
                "accepted scalar Field summary differs from its previewed semantic layout",
            )));
        }
        debug_assert_eq!(&solve, accepted.receipt.report());
        let run_manifest = scalar_elliptic_run_manifest(&accepted.plan, &accepted.receipt)?;
        let field_values = accepted.solution.into_primary_field_values();
        debug_assert_eq!(field_values.len(), field.value_count());
        Ok(ScalarEllipticRunOutcome::Completed(Box::new(
            ScalarEllipticRunResult {
                plan: accepted.plan,
                elapsed: accepted.elapsed,
                field,
                field_values,
                balance,
                assembly,
                run_manifest,
                receipt: accepted.receipt,
            },
        )))
    }

    fn execute_scalar_elliptic_plan_controlled(
        &self,
        accepted: ScalarEllipticRunPlan,
        environment: ScalarEllipticExecutionEnvironment,
        controlled_started: Instant,
        observer: &mut impl ScalarEllipticRunObserver,
    ) -> Result<ControlledScalarEllipticExecution, Vec<Diagnostic>> {
        let replayed = match accepted.mesh().cloned() {
            Some(mesh) => {
                self.preview_scalar_elliptic_run_on_mesh(accepted.intent, environment, mesh)?
            }
            None => self.preview_scalar_elliptic_run(accepted.intent, environment)?,
        };
        if replayed.key != accepted.key
            || replayed.artifact != accepted.artifact
            || replayed.portable != accepted.portable
        {
            return Err(single(capability_error(
                "scalar-elliptic Realization no longer matches its accepted artifact",
            )));
        }

        let binding = DeploymentBinding::bind_host(
            &replayed.portable,
            host_executor(environment, replayed.intent.workers),
        )
        .map_err(single)?;
        if observer.observe(ScalarEllipticRunProgress::PlanReplayed)
            == ScalarEllipticRunDirective::Cancel
        {
            return Ok(ControlledScalarEllipticExecution::Cancelled(Box::new(
                scalar_elliptic_cancellation(
                    replayed,
                    controlled_started,
                    ScalarEllipticRunProgress::PlanReplayed,
                ),
            )));
        }
        let started = Instant::now();
        let workers = replayed.intent.workers;
        let solved = if workers == NonZeroUsize::MIN {
            let (_, finalized) =
                finalize_resolved_scalar_elliptic_cartesian(self.program(), &replayed.resolved)
                    .map_err(single)?;
            solve_finalized_controlled(binding, finalized, &REFERENCE_LINEAR_SOLVER, observer)?
        } else {
            threaded_solve_controlled(self, &replayed, binding, observer)?
        };
        let Some((solution, receipt)) = solved else {
            return Ok(ControlledScalarEllipticExecution::Cancelled(Box::new(
                scalar_elliptic_cancellation(
                    replayed,
                    controlled_started,
                    ScalarEllipticRunProgress::SystemFinalized,
                ),
            )));
        };
        let elapsed = started.elapsed();
        validate_scalar_elliptic_solution(&replayed, &solution, &receipt)?;
        if observer.observe(ScalarEllipticRunProgress::SolutionAccepted)
            == ScalarEllipticRunDirective::Cancel
        {
            return Ok(ControlledScalarEllipticExecution::Cancelled(Box::new(
                scalar_elliptic_cancellation(
                    replayed,
                    controlled_started,
                    ScalarEllipticRunProgress::SolutionAccepted,
                ),
            )));
        }
        Ok(ControlledScalarEllipticExecution::Accepted(Box::new(
            AcceptedScalarEllipticRun {
                plan: replayed,
                elapsed,
                solution,
                receipt,
            },
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_solver::ExecutionTopology;

    const POISSON_2D: &str =
        include_str!("../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");

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
        let mesh =
            ScalarEllipticMesh::uniform(lowered.bounds(), NonZeroUsize::new(4).unwrap()).unwrap();
        assert_eq!(mesh.dimension(), 2);
        assert_eq!(mesh.cells_per_axis().get(), 4);

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

        let incompatible_mesh =
            ScalarEllipticMesh::uniform(&[[0.0, 2.0], [0.0, 1.0]], NonZeroUsize::new(4).unwrap())
                .unwrap();
        let errors = owner
            .preview_scalar_elliptic_run_on_mesh(
                intent(ScalarEllipticMethod::FiniteElement, 4, 1),
                environment,
                incompatible_mesh,
            )
            .unwrap_err();
        assert_eq!(errors[0].code(), codes::INVALID_REALIZATION);

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
        let ControlledScalarEllipticExecution::Accepted(foreign_execution) = foreign_execution
        else {
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
                .run_scalar_elliptic_plan(
                    accepted,
                    ScalarEllipticExecutionEnvironment::host_serial(),
                )
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
        const SOLVER_LIBRARIES: &[ProviderLibrary] =
            &[ProviderLibrary::new("shared-runtime", "1.0.0")];
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
}
