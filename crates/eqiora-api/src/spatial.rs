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
    ScalarEllipticMethod, ScalarEllipticRunCancellation, ScalarEllipticRunDirective,
    ScalarEllipticRunObserver, ScalarEllipticRunOutcome, ScalarEllipticRunPlan,
    ScalarEllipticRunProgress,
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

use eqiora_artifact::{CartesianMeshEnvelopeV1, LayoutArtifacts, RealizationEnvelopeV1};
#[cfg(test)]
use eqiora_artifact::{
    ExecutionProvenanceV1, ExecutionTopologyV1, JsonDecoderLimits, RunManifestV2,
};
#[cfg(all(test, feature = "rayon"))]
use eqiora_backend_rayon::CpuThreadPool;
use eqiora_core::Diagnostic;
#[cfg(test)]
use eqiora_core::diagnostic::codes;
use eqiora_execution::DeploymentBinding;
#[cfg(all(test, feature = "rayon"))]
use eqiora_execution::HostExecutorDescriptor;
use eqiora_meshing::CartesianMesh;
use eqiora_numerics::{
    scalar::finalize_resolved_scalar_elliptic_cartesian, scalar::lower_scalar_elliptic_cartesian,
};
#[cfg(test)]
use eqiora_realization::DiscretizationMethod;
use eqiora_realization::{
    Discretization, ExecutionSchedule, MeshPolicy, RealizationPlan, RealizationRequest,
    RealizationRequirements, RealizationRevision, SemanticRevision, SingleFieldOperatorClaim,
    Target, VectorLayoutKind, resolve,
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

fn generated_cartesian_mesh(
    bounds: &[[f64; 2]],
    cells_per_axis: NonZeroUsize,
) -> Result<CartesianMeshEnvelopeV1, Vec<Diagnostic>> {
    let dimension = NonZeroUsize::new(bounds.len()).ok_or_else(|| {
        single(capability_error(
            "a scalar-elliptic Cartesian Mesh requires at least one dimension",
        ))
    })?;
    if dimension.get() > 3 {
        return Err(single(capability_error(
            "scalar-elliptic Cartesian Meshes admit one through three dimensions",
        )));
    }
    resource_shape(
        ScalarEllipticIntent::new(
            RealizationRevision::new(1),
            ScalarEllipticMethod::FiniteElement,
            cells_per_axis,
            NonZeroUsize::MIN,
        ),
        dimension,
    )?;
    let extents = vec![cells_per_axis.get(); dimension.get()];
    let mesh = CartesianMesh::uniform(bounds, &extents).map_err(single)?;
    CartesianMeshEnvelopeV1::from_mesh(&mesh).map_err(single)
}

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
        let mesh = generated_cartesian_mesh(lowered.bounds(), intent.cells_per_axis)?;
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
        mesh: CartesianMeshEnvelopeV1,
    ) -> Result<ScalarEllipticRunPlan, Vec<Diagnostic>> {
        if (0..mesh.dimension())
            .any(|axis| mesh.mesh().axis_cell_count(axis) != Some(intent.cells_per_axis.get()))
        {
            return Err(single(capability_error(
                "the supplied scalar-elliptic Mesh does not match the spatial policy density",
            )));
        }
        let lowered = lower_scalar_elliptic_cartesian(self.program()).map_err(single)?;
        let expected = generated_cartesian_mesh(lowered.bounds(), intent.cells_per_axis)?;
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
        mesh: Option<CartesianMeshEnvelopeV1>,
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
mod tests;
