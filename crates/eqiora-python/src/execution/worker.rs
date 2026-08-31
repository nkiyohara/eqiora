use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Instant;

use eqiora::Diagnostic;
use eqiora::backends::diffsol::DiffsolTimeBackend;
use eqiora::backends::faer::{FAER_SOLVER_PROVIDER, FaerLinearSolver};
use eqiora::diagnostic::codes;
use eqiora::solver::{LinearSolverBackend, REFERENCE_LINEAR_SOLVER, REFERENCE_SOLVER_PROVIDER};
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
            let provider = request.plan().solver_provider();
            let backend: &dyn LinearSolverBackend = if provider == FAER_SOLVER_PROVIDER {
                &FaerLinearSolver
            } else if provider == REFERENCE_SOLVER_PROVIDER {
                &REFERENCE_LINEAR_SOLVER
            } else {
                return Err(vec![Diagnostic::error(
                    codes::INVALID_REALIZATION,
                    "transient execution rejected a solver provider outside the resolved common Plan",
                )]);
            };
            let outcome = request
                .advance_accepted_actions(backend, |accepted_steps, state| {
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
                })
                .map_err(|diagnostic| vec![diagnostic])?;
            match outcome {
                ControlFlow::Break((accepted_steps, state)) => Ok(NativeWorkerOutcome::Cancelled(
                    NativeRunCancellation::CommonTransient {
                        accepted_steps,
                        maximum_steps,
                        model_time_s: state.time_s(),
                        request_identity: request.identity().to_owned(),
                    },
                )),
                ControlFlow::Continue(states) => {
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
            let outcome = request
                .advance_accepted_actions(&REFERENCE_LINEAR_SOLVER, |accepted_steps, state| {
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
                })
                .map_err(|diagnostic| vec![diagnostic])?;
            match outcome {
                ControlFlow::Break((accepted_steps, state)) => Ok(NativeWorkerOutcome::Cancelled(
                    NativeRunCancellation::CommonTransient {
                        accepted_steps,
                        maximum_steps,
                        model_time_s: state.time_s(),
                        request_identity: request.identity().to_owned(),
                    },
                )),
                ControlFlow::Continue(states) => {
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
