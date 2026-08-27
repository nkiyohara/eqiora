use std::sync::Arc;
use std::time::{Duration, Instant};

use eqiora::Diagnostic;
use eqiora::api::{
    ModelDocument, ReferenceRunDirective, ReferenceRunObserver, ReferenceRunOutcome,
    ReferenceRunPlan, ReferenceRunProgress, ResolvedFixedMeshMonolithicFsiPlan2d,
    ResolvedLinearElasticityPlan2d, ResolvedSteadyStokesPlan2d, ScalarEllipticExecutionEnvironment,
    ScalarEllipticRunDirective, ScalarEllipticRunObserver, ScalarEllipticRunOutcome,
    ScalarEllipticRunPlan, ScalarEllipticRunProgress,
};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use eqiora_numerics::{
    CommonScalarPlan, CommonSpatialPolicy, CommonSteadyStokesPlan, CommonTransientRunRequest,
};

use super::evidence::PyCommonTransientRunProgress;
use super::{
    NativeRunCancellation, NativeRunOutput, NativeRunProgress, RunFailure, RunShared, RunTerminal,
};
use crate::error::catch_native_panic;
use crate::steady_stokes::{SteadyStokesPhysicalEvidence, SteadyStokesRunMaterialization};

const PROGRESS_PUBLICATION_INTERVAL: Duration = Duration::from_millis(100);

struct ReferenceSharedObserver {
    shared: Arc<RunShared>,
    last_publication: Option<Instant>,
}

impl ReferenceSharedObserver {
    fn new(shared: Arc<RunShared>) -> Self {
        Self {
            shared,
            last_publication: None,
        }
    }
}

impl ReferenceRunObserver for ReferenceSharedObserver {
    fn observe(&mut self, progress: ReferenceRunProgress) -> ReferenceRunDirective {
        let cancellation_requested = self.shared.cancellation_requested();
        let now = Instant::now();
        let should_publish =
            progress_publication_due(self.last_publication, now, cancellation_requested);
        if should_publish {
            self.shared
                .publish_progress(NativeRunProgress::Reference(progress));
            self.last_publication = Some(now);
        }
        if cancellation_requested {
            ReferenceRunDirective::Cancel
        } else {
            ReferenceRunDirective::Continue
        }
    }
}

struct ScalarEllipticSharedObserver {
    shared: Arc<RunShared>,
}

impl ScalarEllipticSharedObserver {
    fn new(shared: Arc<RunShared>) -> Self {
        Self { shared }
    }
}

impl ScalarEllipticRunObserver for ScalarEllipticSharedObserver {
    fn observe(&mut self, progress: ScalarEllipticRunProgress) -> ScalarEllipticRunDirective {
        self.shared
            .publish_progress(NativeRunProgress::ScalarElliptic(progress));
        if self.shared.cancellation_requested() {
            ScalarEllipticRunDirective::Cancel
        } else {
            ScalarEllipticRunDirective::Continue
        }
    }
}

pub(super) fn progress_publication_due(
    last_publication: Option<Instant>,
    now: Instant,
    cancellation_requested: bool,
) -> bool {
    cancellation_requested
        || last_publication
            .is_none_or(|last| now.duration_since(last) >= PROGRESS_PUBLICATION_INTERVAL)
}

#[derive(Debug)]
pub(super) enum NativeRunJob {
    Reference {
        document: ModelDocument,
        plan: ReferenceRunPlan,
    },
    ScalarElliptic {
        document: ModelDocument,
        plan: Box<ScalarEllipticRunPlan>,
        environment: ScalarEllipticExecutionEnvironment,
    },
    SteadyStokes(Box<ResolvedSteadyStokesPlan2d>),
    LinearElasticity(Box<ResolvedLinearElasticityPlan2d>),
    FixedMeshMonolithic(Box<ResolvedFixedMeshMonolithicFsiPlan2d>),
    CommonScalar(Box<CommonScalarPlan>),
    CommonSteadyStokes(Box<CommonSteadyStokesPlan>),
    CommonTransient(Box<CommonTransientRunRequest>),
}

enum NativeWorkerOutcome {
    Completed(NativeRunOutput),
    Cancelled(NativeRunCancellation),
}

enum AcceptedStepOutcome<S> {
    Completed(Vec<(usize, S)>),
    Cancelled { accepted_steps: usize, state: S },
}

/// Private deterministic controller seam for accepted-step scheduling.
///
/// The controller is observed only before step one and after a fully accepted
/// step. Production supplies the shared cancellation flag; tests inject exact
/// boundary decisions without racing a worker thread or creating a public event API.
fn run_accepted_steps<S: Clone, E>(
    initial: S,
    maximum_steps: usize,
    output_steps: &[usize],
    mut advance: impl FnMut(S) -> Result<S, E>,
    mut cancel_at_boundary: impl FnMut(usize, &S) -> bool,
) -> Result<AcceptedStepOutcome<S>, E> {
    let mut state = initial;
    if cancel_at_boundary(0, &state) {
        return Ok(AcceptedStepOutcome::Cancelled {
            accepted_steps: 0,
            state,
        });
    }
    let mut outputs = Vec::with_capacity(output_steps.len());
    for accepted_steps in 1..=maximum_steps {
        state = advance(state)?;
        if output_steps.binary_search(&accepted_steps).is_ok() {
            outputs.push((accepted_steps, state.clone()));
        }
        if cancel_at_boundary(accepted_steps, &state) {
            return Ok(AcceptedStepOutcome::Cancelled {
                accepted_steps,
                state,
            });
        }
    }
    Ok(AcceptedStepOutcome::Completed(outputs))
}

fn execute_job(
    job: NativeRunJob,
    shared: &Arc<RunShared>,
) -> Result<NativeWorkerOutcome, Vec<Diagnostic>> {
    match job {
        NativeRunJob::Reference { document, plan } => {
            let mut observer = ReferenceSharedObserver::new(Arc::clone(shared));
            match document.run_reference_plan_controlled(plan, &mut observer)? {
                ReferenceRunOutcome::Completed(result) => Ok(NativeWorkerOutcome::Completed(
                    NativeRunOutput::Reference(result),
                )),
                ReferenceRunOutcome::Cancelled(cancellation) => Ok(NativeWorkerOutcome::Cancelled(
                    NativeRunCancellation::Reference(cancellation),
                )),
            }
        }
        NativeRunJob::ScalarElliptic {
            document,
            plan,
            environment,
        } => {
            let mut observer = ScalarEllipticSharedObserver::new(Arc::clone(shared));
            match document.run_scalar_elliptic_plan_controlled(*plan, environment, &mut observer)? {
                ScalarEllipticRunOutcome::Completed(result) => Ok(NativeWorkerOutcome::Completed(
                    NativeRunOutput::ScalarElliptic(result),
                )),
                ScalarEllipticRunOutcome::Cancelled(cancellation) => {
                    Ok(NativeWorkerOutcome::Cancelled(
                        NativeRunCancellation::ScalarElliptic(cancellation),
                    ))
                }
            }
        }
        NativeRunJob::SteadyStokes(plan) => {
            let started = Instant::now();
            let result = plan
                .execute(&FaerLinearSolver)
                .map_err(|diagnostic| vec![diagnostic])?;
            let elapsed_seconds = started.elapsed().as_secs_f64();
            let physical = SteadyStokesPhysicalEvidence::new(
                result.cylinder_force_on_fluid(),
                result.inlet_flux(),
                result.outlet_flux(),
                result.net_flux(),
                result.momentum_closure(),
            );
            let materialized = SteadyStokesRunMaterialization::new(
                result.run().clone(),
                result.snapshot().clone(),
                result.pressure_projection().clone(),
                result.solution().clone(),
                physical,
            );
            Ok(NativeWorkerOutcome::Completed(
                NativeRunOutput::SteadyStokes {
                    result: Box::new(materialized),
                    elapsed_seconds,
                },
            ))
        }
        NativeRunJob::LinearElasticity(plan) => {
            let started = Instant::now();
            let result = plan
                .execute(&REFERENCE_LINEAR_SOLVER)
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(
                NativeRunOutput::LinearElasticity {
                    result: Box::new(result),
                    elapsed_seconds: started.elapsed().as_secs_f64(),
                },
            ))
        }
        NativeRunJob::FixedMeshMonolithic(plan) => {
            let started = Instant::now();
            let result = plan
                .execute(&REFERENCE_LINEAR_SOLVER)
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(
                NativeRunOutput::FixedMeshMonolithic {
                    result: Box::new(result),
                    elapsed_seconds: started.elapsed().as_secs_f64(),
                },
            ))
        }
        NativeRunJob::CommonScalar(plan) => {
            let started = Instant::now();
            let result = plan.run().map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(
                NativeRunOutput::CommonScalar {
                    result: Box::new(result),
                    elapsed_seconds: started.elapsed().as_secs_f64(),
                },
            ))
        }
        NativeRunJob::CommonSteadyStokes(plan) => {
            let started = Instant::now();
            let result = plan
                .run(&FaerLinearSolver)
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(
                NativeRunOutput::CommonSteadyStokes {
                    result: Box::new(result),
                    elapsed_seconds: started.elapsed().as_secs_f64(),
                },
            ))
        }
        NativeRunJob::CommonTransient(request) => {
            let started = Instant::now();
            let maximum_steps = request.accepted_steps().get();
            let outcome = run_accepted_steps(
                request.state().clone(),
                maximum_steps,
                request.output_steps(),
                |state| match request.plan().spatial() {
                    CommonSpatialPolicy::MiniP1 => {
                        request.plan().advance_one(&state, &FaerLinearSolver)
                    }
                    CommonSpatialPolicy::CellCentered => {
                        request.plan().advance_one(&state, &REFERENCE_LINEAR_SOLVER)
                    }
                    _ => unreachable!("closed transient spatial policy"),
                },
                |accepted_steps, state| {
                    if accepted_steps > 0 {
                        shared.publish_progress(NativeRunProgress::CommonTransient(
                            PyCommonTransientRunProgress {
                                accepted_steps,
                                maximum_steps,
                                model_time_bits: state.time_s().to_bits(),
                            },
                        ));
                    }
                    shared.cancellation_requested()
                },
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            match outcome {
                AcceptedStepOutcome::Cancelled {
                    accepted_steps,
                    state,
                } => Ok(NativeWorkerOutcome::Cancelled(
                    NativeRunCancellation::CommonTransient {
                        accepted_steps,
                        maximum_steps,
                        model_time_s: state.time_s(),
                        request_identity: request.identity().to_owned(),
                    },
                )),
                AcceptedStepOutcome::Completed(states) => Ok(NativeWorkerOutcome::Completed(
                    NativeRunOutput::CommonTransient {
                        states,
                        elapsed_seconds: started.elapsed().as_secs_f64(),
                    },
                )),
            }
        }
    }
}

pub(super) fn run_worker(job: NativeRunJob, shared: Arc<RunShared>) {
    shared.mark_running();
    let outcome = catch_native_panic(|| execute_job(job, &shared));
    match outcome {
        Ok(Ok(NativeWorkerOutcome::Completed(result))) => {
            shared.finish(RunTerminal::Completed(Some(result)));
        }
        Ok(Ok(NativeWorkerOutcome::Cancelled(cancellation))) => {
            shared.finish(RunTerminal::Cancelled(cancellation));
        }
        Ok(Err(diagnostics)) => {
            shared.finish(RunTerminal::Failed(RunFailure::Execution(diagnostics)));
        }
        Err(diagnostic) => {
            shared.finish(RunTerminal::Failed(RunFailure::Internal(vec![diagnostic])));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AcceptedStepOutcome, progress_publication_due, run_accepted_steps};

    #[test]
    fn progress_policy_coalesces_until_the_interval_or_cancellation() {
        let start = std::time::Instant::now();
        assert!(progress_publication_due(None, start, false));
        assert!(!progress_publication_due(
            Some(start),
            start + std::time::Duration::from_millis(99),
            false
        ));
        assert!(progress_publication_due(
            Some(start),
            start + std::time::Duration::from_millis(100),
            false
        ));
        assert!(progress_publication_due(Some(start), start, true));
    }

    #[test]
    fn injected_controller_cancels_only_at_exact_accepted_boundaries() {
        for cancellation_boundary in [0, 1] {
            let mut observed = Vec::new();
            let outcome = run_accepted_steps(
                0_u64,
                3,
                &[1, 2, 3],
                |state| Ok::<_, ()>(state + 1),
                |accepted, state| {
                    observed.push((accepted, *state));
                    accepted == cancellation_boundary
                },
            )
            .unwrap();
            let AcceptedStepOutcome::Cancelled {
                accepted_steps,
                state,
            } = outcome
            else {
                panic!("injected cancellation returned partial success")
            };
            assert_eq!(accepted_steps, cancellation_boundary);
            assert_eq!(state, cancellation_boundary as u64);
            assert_eq!(observed.last(), Some(&(cancellation_boundary, state)));
        }
    }
}
