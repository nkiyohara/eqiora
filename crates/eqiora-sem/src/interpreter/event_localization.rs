use super::{
    BTreeMap, Diagnostic, EvalContext, EventTask, ExecutionPlan, ExpressionBackend, KernelProgram,
    ReferenceConfig, RuntimeState, codes, evaluate, event, execution_error, kernel_path,
    solve_continuous_step,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn crossing_events(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    start_state: &RuntimeState,
    end_state: &RuntimeState,
    start: f64,
    end: f64,
    config: ReferenceConfig,
    backend: &impl ExpressionBackend,
) -> Result<Vec<usize>, Diagnostic> {
    let mut crossings = Vec::new();
    for (index, task) in plan.events.iter().enumerate() {
        let before = evaluate_event_guard(program, plan, task, start_state, start, backend)?;
        let after = evaluate_event_guard(program, plan, task, end_state, end, backend)?;
        if event::crosses(task.direction, before, after, config.event_guard_tolerance) {
            crossings.push(index);
        }
    }
    Ok(crossings)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn locate_event_time(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    start_state: &RuntimeState,
    end_state: &RuntimeState,
    start: f64,
    end: f64,
    event_index: usize,
    config: ReferenceConfig,
    backend: &impl ExpressionBackend,
) -> Result<f64, Diagnostic> {
    let task = &plan.events[event_index];
    let mut left_time = start;
    let mut right_time = end;
    let mut left_guard = evaluate_event_guard(program, plan, task, start_state, start, backend)?;
    let right_guard = evaluate_event_guard(program, plan, task, end_state, end, backend)?;
    if !event::crosses(
        task.direction,
        left_guard,
        right_guard,
        config.event_guard_tolerance,
    ) {
        return Err(execution_error(
            "event localization received a bracket without the requested crossing",
            start,
        ));
    }

    for _ in 0..config.max_event_localization_iterations {
        if right_time - left_time <= event_time_tolerance(left_time, right_time, config) {
            return Ok(left_time + 0.5 * (right_time - left_time));
        }
        let midpoint = left_time + 0.5 * (right_time - left_time);
        if midpoint <= left_time || midpoint >= right_time {
            return Ok(midpoint);
        }
        let mut midpoint_state = start_state.clone();
        solve_continuous_step(
            program,
            plan,
            &mut midpoint_state,
            start,
            midpoint,
            config,
            backend,
        )?;
        let midpoint_guard =
            evaluate_event_guard(program, plan, task, &midpoint_state, midpoint, backend)?;
        if event::root_is_left_of(
            task.direction,
            left_guard,
            midpoint_guard,
            config.event_guard_tolerance,
        ) {
            right_time = midpoint;
        } else {
            left_time = midpoint;
            left_guard = midpoint_guard;
        }
    }

    Err(Diagnostic::error(
        codes::NONLINEAR_SOLVE_FAILED,
        format!(
            "event root localization exceeded {} iterations",
            config.max_event_localization_iterations
        ),
    )
    .with_graph_path(kernel_path(task.activation)))
}

fn evaluate_event_guard(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    task: &EventTask,
    state: &RuntimeState,
    time: f64,
    backend: &impl ExpressionBackend,
) -> Result<f64, Diagnostic> {
    let empty = BTreeMap::new();
    let empty_physical = BTreeMap::new();
    let context = EvalContext {
        program,
        time,
        fields: &state.fields,
        field_candidates: &empty,
        derivatives: &state.derivatives,
        next_fields: &empty,
        ports: &state.ports,
        port_candidates: &empty,
        signal_sources: &plan.signal_sources,
        physical: &state.physical,
        physical_candidates: &empty_physical,
    };
    let values = backend.evaluate(task.activation, &task.guard, &mut |symbol| {
        evaluate::resolve_symbol(symbol, &context)
    })?;
    match values.as_slice() {
        [value] => Ok(*value),
        _ => Err(execution_error(
            "validated event guard did not produce exactly one value",
            time,
        )),
    }
}

fn event_time_tolerance(left: f64, right: f64, config: ReferenceConfig) -> f64 {
    config
        .event_time_tolerance
        .max(64.0 * f64::EPSILON * left.abs().max(right.abs()).max(config.max_step).max(1.0))
}
