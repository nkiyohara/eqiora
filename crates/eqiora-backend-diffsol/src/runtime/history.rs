//! Copy accepted native stencils while their dense output remains available.
use super::*;
use diffsol::OdeEquations;
use eqiora_time::{AcceptedTimeHistory, TimeHistoryStep};

pub(super) struct CapturedSolution {
    primal: TimeSolution,
    sensitivities: Vec<f64>,
    sensitivity_history: Option<AcceptedTimeHistory>,
}

impl CapturedSolution {
    pub(super) fn primal(self) -> TimeSolution {
        self.primal
    }
    pub(super) fn sensitivities(
        self,
        count: usize,
    ) -> Result<ForwardSensitivitySolution, Diagnostic> {
        ForwardSensitivitySolution::accepted_with_history(
            self.primal,
            count,
            self.sensitivities,
            self.sensitivity_history
                .ok_or_else(|| solve_failed("missing native sensitivity history"))?,
        )
    }
}

pub(super) fn capture<'a, E, S>(
    solver: &mut S,
    problem: &TimeProblem<'_>,
    plan: &TimePlan,
    parameters: usize,
    failures: &CallbackFailures,
) -> Result<CapturedSolution, Diagnostic>
where
    E: OdeEquations<T = f64, V = NalgebraVec<f64>, M = NalgebraMat<f64>> + 'a,
    S: OdeSolverMethod<'a, E>,
{
    let dimension = problem.dimension();
    let times = plan.output_times();
    let final_time = *times.last().expect("validated output times");
    solver
        .set_stop_time(final_time)
        .map_err(|error| map_failure(failures, "set Diffsol history horizon", error))?;
    if problem.initial_condition() == eqiora_time::InitialConditionPolicy::Provided
        && collect_vector(solver.state().y) != problem.initial_state()
    {
        return Err(solve_failed(
            "native initialization changed a provided initial state",
        ));
    }
    let mut steps = Vec::new();
    let mut sensitivity_steps = Vec::new();
    let mut values = Vec::with_capacity(dimension * times.len());
    let mut sensitivities = vec![0.0; parameters * dimension * times.len()];
    let mut sample = 0;
    loop {
        let start_time = solver.state().t;
        let start_state = collect_vector(solver.state().y);
        let start_sensitivity = flatten(solver.state().s);
        let stop = solver
            .step()
            .map_err(|error| map_failure(failures, "advance Diffsol accepted history", error))?;
        if matches!(stop, OdeSolverStopReason::RootFound(..)) {
            return Err(solve_failed(
                "ordinary accepted history cannot commit a root reset",
            ));
        }
        let end_time = solver.state().t;
        let midpoint = start_time + (end_time - start_time) * 0.5;
        let middle = solver
            .interpolate(midpoint)
            .map_err(|error| map_failure(failures, "capture native Diffsol midpoint", error))?;
        steps.push(TimeHistoryStep::accepted(
            start_time,
            end_time,
            start_state,
            collect_vector(&middle),
            collect_vector(solver.state().y),
        )?);
        if parameters > 0 {
            let middle = solver.interpolate_sens(midpoint).map_err(|error| {
                map_failure(
                    failures,
                    "capture native Diffsol sensitivity midpoint",
                    error,
                )
            })?;
            sensitivity_steps.push(TimeHistoryStep::accepted(
                start_time,
                end_time,
                start_sensitivity,
                flatten(&middle),
                flatten(solver.state().s),
            )?);
        }
        while sample < times.len() && times[sample] <= end_time {
            let step = steps.last().expect("captured native step");
            if let Some(state) = stencil_state(step, times[sample]) {
                values.extend_from_slice(state);
            } else {
                let state = solver.interpolate(times[sample]).map_err(|error| {
                    map_failure(failures, "sample native Diffsol history", error)
                })?;
                values.extend(collect_vector(&state));
            }
            if parameters > 0 {
                let step = sensitivity_steps.last().expect("captured sensitivity step");
                let interpolated;
                let state = if let Some(state) = stencil_state(step, times[sample]) {
                    state
                } else {
                    let native = solver.interpolate_sens(times[sample]).map_err(|error| {
                        map_failure(failures, "sample native Diffsol sensitivities", error)
                    })?;
                    interpolated = flatten(&native);
                    &interpolated
                };
                if state.len() != parameters * dimension {
                    return Err(solve_failed("unexpected native sensitivity count"));
                }
                for (parameter, state) in state.chunks_exact(dimension).enumerate() {
                    let start = (parameter * times.len() + sample) * dimension;
                    sensitivities[start..start + dimension].copy_from_slice(state);
                }
            }
            sample += 1;
        }
        if stop == OdeSolverStopReason::TstopReached {
            break;
        }
    }
    if sample != times.len() || steps[0].start_time() != plan.start_time() {
        return Err(solve_failed(
            "native accepted history does not span the requested solve",
        ));
    }
    let history = AcceptedTimeHistory::accepted(dimension, steps)?;
    let primal = TimeSolution::accepted_with_history(
        dimension,
        times.to_vec(),
        values,
        TimeExecutionReport::new(
            DIFFSOL_TIME_BACKEND,
            plan.method(),
            problem.equation_class(),
            problem.initial_condition(),
        ),
        history,
    )?;
    let sensitivity_history = if parameters == 0 {
        None
    } else {
        Some(AcceptedTimeHistory::accepted(
            parameters * dimension,
            sensitivity_steps,
        )?)
    };
    Ok(CapturedSolution {
        primal,
        sensitivities,
        sensitivity_history,
    })
}

fn flatten(states: &[NalgebraVec<f64>]) -> Vec<f64> {
    states.iter().flat_map(collect_vector).collect()
}

fn stencil_state(step: &TimeHistoryStep, time: f64) -> Option<&[f64]> {
    if time == step.start_time() {
        Some(step.start_state())
    } else if time == step.end_time() {
        Some(step.end_state())
    } else if time == step.start_time() + (step.end_time() - step.start_time()) * 0.5 {
        Some(step.midpoint_state())
    } else {
        None
    }
}
