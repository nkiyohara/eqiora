use std::sync::Arc;
use std::time::{Duration, Instant};

use eqiora::Diagnostic;
use eqiora::api::{
    ModelDocument, ReferenceRunDirective, ReferenceRunObserver, ReferenceRunOutcome,
    ReferenceRunPlan, ReferenceRunProgress, ResolvedSteadyStokesPlan2d,
    ScalarEllipticExecutionEnvironment, ScalarEllipticRunDirective, ScalarEllipticRunObserver,
    ScalarEllipticRunOutcome, ScalarEllipticRunPlan, ScalarEllipticRunProgress,
};
use eqiora::backends::faer::FaerLinearSolver;

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
    use super::progress_publication_due;

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
}
