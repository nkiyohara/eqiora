use std::sync::Arc;
use std::time::Instant;

use eqiora::Diagnostic;
use eqiora::backends::diffsol::DiffsolTimeBackend;
use eqiora::backends::faer::{FAER_SOLVER_PROVIDER, FaerLinearSolver};
use eqiora::diagnostic::codes;
use eqiora::solver::{REFERENCE_LINEAR_SOLVER, REFERENCE_SOLVER_PROVIDER};
use eqiora_numerics::{
    CommonElasticityPlan, CommonFsiRunRequest, CommonOdeRunRequest, CommonResult, CommonScalarPlan,
    CommonSteadyStokesPlan, CommonTrajectory, CommonTransientRunRequest,
};

use super::evidence::PyCommonTransientRunProgress;
use super::{
    NativeRunCancellation, NativeRunOutput, NativeRunProgress, RunFailure, RunShared, RunTerminal,
};
use crate::error::catch_native_panic;

#[derive(Debug)]
pub(super) enum NativeRunJob {
    Scalar(Box<CommonScalarPlan>),
    Elasticity(Box<CommonElasticityPlan>),
    SteadyStokes(Box<CommonSteadyStokesPlan>),
    Transient(Box<CommonTransientRunRequest>),
    Fsi(Box<CommonFsiRunRequest>),
    Ode(Box<CommonOdeRunRequest>),
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
        NativeRunJob::Scalar(plan) => {
            let started = Instant::now();
            let result = plan
                .run_result()
                .and_then(|result| result.with_elapsed_seconds(started.elapsed().as_secs_f64()))
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(NativeRunOutput::Result(
                Box::new(result),
            )))
        }
        NativeRunJob::Elasticity(plan) => {
            let started = Instant::now();
            let result = plan
                .run_result()
                .and_then(|result| result.with_elapsed_seconds(started.elapsed().as_secs_f64()))
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(NativeRunOutput::Result(
                Box::new(result),
            )))
        }
        NativeRunJob::SteadyStokes(plan) => {
            let started = Instant::now();
            let result = plan
                .run_result(&FaerLinearSolver)
                .map_err(|diagnostic| vec![diagnostic])?;
            let result = result
                .with_elapsed_seconds(started.elapsed().as_secs_f64())
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(NativeRunOutput::Result(
                Box::new(result),
            )))
        }
        NativeRunJob::Transient(request) => {
            let started = Instant::now();
            let maximum_steps = request.accepted_steps().get();
            let outcome = run_accepted_steps(
                request.state().clone(),
                maximum_steps,
                request.output_steps(),
                |state| {
                    let provider = request.plan().solver_provider();
                    if provider == FAER_SOLVER_PROVIDER {
                        request.plan().advance_one(&state, &FaerLinearSolver)
                    } else if provider == REFERENCE_SOLVER_PROVIDER {
                        request.plan().advance_one(&state, &REFERENCE_LINEAR_SOLVER)
                    } else {
                        Err(Diagnostic::error(
                            codes::INVALID_REALIZATION,
                            "transient execution rejected a solver provider outside the resolved common Plan",
                        ))
                    }
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
                AcceptedStepOutcome::Completed(states) => {
                    let trajectory = CommonTrajectory::accept_transient_flow(*request, states)
                        .map_err(|diagnostic| vec![diagnostic])?;
                    let result = CommonResult::accept_trajectory(
                        started.elapsed().as_secs_f64(),
                        trajectory,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    Ok(NativeWorkerOutcome::Completed(NativeRunOutput::Result(
                        Box::new(result),
                    )))
                }
            }
        }
        NativeRunJob::Fsi(request) => {
            let started = Instant::now();
            let maximum_steps = request.accepted_steps().get();
            let outcome = run_accepted_steps(
                request.state().clone(),
                maximum_steps,
                request.output_steps(),
                |state| request.plan().advance(&state, &REFERENCE_LINEAR_SOLVER),
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
                AcceptedStepOutcome::Completed(states) => {
                    let trajectory = CommonTrajectory::accept_fsi(*request, states)
                        .map_err(|diagnostic| vec![diagnostic])?;
                    let result = CommonResult::accept_trajectory(
                        started.elapsed().as_secs_f64(),
                        trajectory,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    Ok(NativeWorkerOutcome::Completed(NativeRunOutput::Result(
                        Box::new(result),
                    )))
                }
            }
        }
        NativeRunJob::Ode(request) => {
            let started = Instant::now();
            let problem = request.problem().map_err(|diagnostic| vec![diagnostic])?;
            let solution = DiffsolTimeBackend::new()
                .solve(&problem, request.time_plan())
                .map_err(|diagnostic| vec![diagnostic])?;
            let trajectory = CommonTrajectory::accept_ode(*request, solution)
                .map_err(|diagnostic| vec![diagnostic])?;
            let result =
                CommonResult::accept_trajectory(started.elapsed().as_secs_f64(), trajectory)
                    .map_err(|diagnostic| vec![diagnostic])?;
            Ok(NativeWorkerOutcome::Completed(NativeRunOutput::Result(
                Box::new(result),
            )))
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
    use super::{AcceptedStepOutcome, run_accepted_steps};

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
