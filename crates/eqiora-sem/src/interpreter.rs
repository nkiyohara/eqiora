//! Deterministic reference execution for scalar continuous/periodic models.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DynQuantity, GraphPath, RawId};
use eqiora_schema::kernel::{
    ActivationKind, ClockKind, ConnectionSemantics, DomainKind, ExprNode, KernelNode, RationalTime,
    SignalDirection, SymbolRef,
};

use crate::evaluate::{self, EvalContext, ReferenceExpressionBackend};
use crate::event::{self, EventTask};
use crate::solver::{self, NonlinearSettings};
use crate::{
    ComposedResidualSystem, ExpressionBackend, Interpreter, KernelProgram, PhysicalSample,
    PhysicalUnknown, Sample, Trajectory,
};

/// One accepted semantic-execution boundary visible to control-plane clients.
///
/// Observation never occurs inside expression evaluation, Newton iteration,
/// event localization, or an atomic activation commit. Model-time execution
/// therefore cannot be interrupted in a partially accepted state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutionProgress {
    model_time: f64,
    end_time: f64,
    accepted_steps: usize,
    maximum_steps: usize,
}

impl ExecutionProgress {
    const fn new(
        model_time: f64,
        end_time: f64,
        accepted_steps: usize,
        maximum_steps: usize,
    ) -> Self {
        Self {
            model_time,
            end_time,
            accepted_steps,
            maximum_steps,
        }
    }

    /// Last fully accepted model time in seconds.
    #[must_use]
    pub const fn model_time(self) -> f64 {
        self.model_time
    }

    /// Requested inclusive final model time in seconds.
    #[must_use]
    pub const fn end_time(self) -> f64 {
        self.end_time
    }

    /// Number of fully accepted time/event steps.
    #[must_use]
    pub const fn accepted_steps(self) -> usize {
        self.accepted_steps
    }

    /// Configured accepted-step safety limit.
    #[must_use]
    pub const fn maximum_steps(self) -> usize {
        self.maximum_steps
    }
}

fn accepted_progress(time: f64, steps: usize, config: ReferenceConfig) -> ExecutionProgress {
    ExecutionProgress::new(time, config.end_time, steps, config.max_steps)
}

/// Control-plane decision returned only at an accepted execution boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionDirective {
    /// Continue from the accepted state.
    Continue,
    /// Stop and return the accepted boundary without producing a trajectory.
    Cancel,
}

/// Synchronous observer for accepted semantic-execution boundaries.
///
/// Implementations must remain bounded. Presentation adapters should
/// coalesce progress before crossing an IPC boundary; this callback is not a
/// numerical inner-loop extension point.
pub trait ExecutionObserver {
    /// Inspect one accepted boundary and decide whether execution continues.
    fn observe(&mut self, progress: ExecutionProgress) -> ExecutionDirective;
}

/// Terminal outcome of explicitly controlled reference execution.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionOutcome {
    /// The complete requested interval was accepted.
    Completed(Trajectory),
    /// Cancellation was observed at this fully accepted boundary.
    Cancelled(ExecutionProgress),
}

#[derive(Debug, Default)]
struct Uninterrupted;

impl ExecutionObserver for Uninterrupted {
    fn observe(&mut self, _progress: ExecutionProgress) -> ExecutionDirective {
        ExecutionDirective::Continue
    }
}

/// Numerical controls for the deliberately conservative reference evaluator.
///
/// These values choose an approximation; they are not part of model meaning.
/// Continuous relations use backward Euler and a dense finite-difference
/// Newton solve so the implementation remains small and auditable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceConfig {
    end_time: f64,
    max_step: f64,
    absolute_tolerance: f64,
    relative_tolerance: f64,
    max_nonlinear_iterations: usize,
    max_steps: usize,
    event_time_tolerance: f64,
    event_guard_tolerance: f64,
    max_event_localization_iterations: usize,
    max_zero_time_events: usize,
}

impl ReferenceConfig {
    /// Create a run from model time zero through `end_time` seconds.
    ///
    /// # Errors
    /// Returns `EQ0501` unless times are finite, `end_time` is non-negative,
    /// and `max_step` is strictly positive.
    pub fn new(end_time: f64, max_step: f64) -> Result<Self, Diagnostic> {
        let config = Self {
            end_time,
            max_step,
            absolute_tolerance: 1.0e-10,
            relative_tolerance: 1.0e-10,
            max_nonlinear_iterations: 32,
            max_steps: 1_000_000,
            event_time_tolerance: 1.0e-10,
            event_guard_tolerance: 1.0e-10,
            max_event_localization_iterations: 80,
            max_zero_time_events: 64,
        };
        config.validate()?;
        Ok(config)
    }

    /// Override Newton residual tolerances.
    ///
    /// # Errors
    /// Returns `EQ0501` unless both are finite, the absolute tolerance is
    /// positive, and the relative tolerance is non-negative.
    pub fn with_nonlinear_tolerances(
        mut self,
        absolute: f64,
        relative: f64,
    ) -> Result<Self, Diagnostic> {
        self.absolute_tolerance = absolute;
        self.relative_tolerance = relative;
        self.validate()?;
        Ok(self)
    }

    /// Override safety limits for nonlinear iterations and total time steps.
    ///
    /// # Errors
    /// Returns `EQ0501` when either limit is zero.
    pub fn with_limits(
        mut self,
        max_nonlinear_iterations: usize,
        max_steps: usize,
    ) -> Result<Self, Diagnostic> {
        self.max_nonlinear_iterations = max_nonlinear_iterations;
        self.max_steps = max_steps;
        self.validate()?;
        Ok(self)
    }

    /// Override event root tolerances in model seconds and guard units.
    ///
    /// # Errors
    /// Returns `EQ0501` unless both values are finite and strictly positive.
    pub fn with_event_tolerances(mut self, time: f64, guard: f64) -> Result<Self, Diagnostic> {
        self.event_time_tolerance = time;
        self.event_guard_tolerance = guard;
        self.validate()?;
        Ok(self)
    }

    /// Override event localization and zero-time iteration limits.
    ///
    /// # Errors
    /// Returns `EQ0501` when either limit is zero.
    pub fn with_event_limits(
        mut self,
        localization_iterations: usize,
        zero_time_events: usize,
    ) -> Result<Self, Diagnostic> {
        self.max_event_localization_iterations = localization_iterations;
        self.max_zero_time_events = zero_time_events;
        self.validate()?;
        Ok(self)
    }

    /// Inclusive final model time in seconds.
    #[must_use]
    pub const fn end_time(self) -> f64 {
        self.end_time
    }

    /// Maximum backward-Euler step in seconds.
    #[must_use]
    pub const fn max_step(self) -> f64 {
        self.max_step
    }

    /// Absolute residual tolerance used by the reference Newton solve.
    #[must_use]
    pub const fn absolute_tolerance(self) -> f64 {
        self.absolute_tolerance
    }

    /// Relative residual tolerance used by the reference Newton solve.
    #[must_use]
    pub const fn relative_tolerance(self) -> f64 {
        self.relative_tolerance
    }

    /// Maximum Newton iterations admitted for one implicit solve.
    #[must_use]
    pub const fn max_nonlinear_iterations(self) -> usize {
        self.max_nonlinear_iterations
    }

    /// Maximum accepted model-time steps for one reference run.
    #[must_use]
    pub const fn max_steps(self) -> usize {
        self.max_steps
    }

    /// Event-time localization tolerance in model seconds.
    #[must_use]
    pub const fn event_time_tolerance(self) -> f64 {
        self.event_time_tolerance
    }

    /// Zero-crossing guard tolerance in guard units.
    #[must_use]
    pub const fn event_guard_tolerance(self) -> f64 {
        self.event_guard_tolerance
    }

    /// Maximum bisection/localization iterations for one event proposal.
    #[must_use]
    pub const fn max_event_localization_iterations(self) -> usize {
        self.max_event_localization_iterations
    }

    /// Maximum consecutive zero-model-time event commits.
    #[must_use]
    pub const fn max_zero_time_events(self) -> usize {
        self.max_zero_time_events
    }

    fn validate(self) -> Result<(), Diagnostic> {
        if !self.end_time.is_finite() || self.end_time < 0.0 {
            return Err(config_error(
                "reference end_time must be finite and non-negative",
            ));
        }
        if !self.max_step.is_finite() || self.max_step <= 0.0 {
            return Err(config_error(
                "reference max_step must be finite and strictly positive",
            ));
        }
        if !self.absolute_tolerance.is_finite() || self.absolute_tolerance <= 0.0 {
            return Err(config_error(
                "Newton absolute tolerance must be finite and strictly positive",
            ));
        }
        if !self.relative_tolerance.is_finite() || self.relative_tolerance < 0.0 {
            return Err(config_error(
                "Newton relative tolerance must be finite and non-negative",
            ));
        }
        if self.max_nonlinear_iterations == 0 || self.max_steps == 0 {
            return Err(config_error("reference iteration limits must be non-zero"));
        }
        if !self.event_time_tolerance.is_finite() || self.event_time_tolerance <= 0.0 {
            return Err(config_error(
                "event time tolerance must be finite and strictly positive",
            ));
        }
        if !self.event_guard_tolerance.is_finite() || self.event_guard_tolerance <= 0.0 {
            return Err(config_error(
                "event guard tolerance must be finite and strictly positive",
            ));
        }
        if self.max_event_localization_iterations == 0 || self.max_zero_time_events == 0 {
            return Err(config_error("reference event limits must be non-zero"));
        }
        Ok(())
    }

    const fn nonlinear_settings(self) -> NonlinearSettings {
        NonlinearSettings {
            absolute_tolerance: self.absolute_tolerance,
            relative_tolerance: self.relative_tolerance,
            max_iterations: self.max_nonlinear_iterations,
        }
    }
}

impl Interpreter {
    /// Evaluate a validated model with deterministic reference numerics.
    ///
    /// Periodic activations at the same exact rational instant are solved as
    /// one simultaneous system. At an activation instant, `Field` and `Pre`
    /// read the pre-activation state while `Next` values commit atomically.
    /// Causal signal inputs alias their one output and periodic outputs hold
    /// their value between ticks.
    ///
    /// # Errors
    /// Returns structured diagnostics for missing initial/input values,
    /// unsupported semantic combinations, non-square systems, non-finite
    /// expressions, exact-clock overflow, or failed nonlinear solves.
    pub fn run(
        &self,
        program: &KernelProgram,
        config: ReferenceConfig,
    ) -> Result<Trajectory, Vec<Diagnostic>> {
        let mut observer = Uninterrupted;
        match self.run_controlled_with_expression_backend(
            program,
            config,
            &ReferenceExpressionBackend,
            &mut observer,
        )? {
            ExecutionOutcome::Completed(trajectory) => Ok(trajectory),
            ExecutionOutcome::Cancelled(_) => {
                unreachable!("the uninterrupted observer cannot request cancellation")
            }
        }
    }

    /// Evaluate a validated model with cancellation observed only at accepted
    /// semantic-execution boundaries.
    ///
    /// A cancelled outcome contains no partial trajectory. Its progress value
    /// identifies the last atomically accepted boundary for control-plane
    /// evidence and restart decisions.
    ///
    /// # Errors
    /// Returns the same structured diagnostics as [`Self::run`].
    pub fn run_controlled(
        &self,
        program: &KernelProgram,
        config: ReferenceConfig,
        observer: &mut impl ExecutionObserver,
    ) -> Result<ExecutionOutcome, Vec<Diagnostic>> {
        self.run_controlled_with_expression_backend(
            program,
            config,
            &ReferenceExpressionBackend,
            observer,
        )
    }

    /// Run the reference activation/numerical engine with an independently
    /// lowered expression backend. This is a conformance hook, not a way to
    /// redefine canonical semantics.
    #[doc(hidden)]
    pub fn run_with_expression_backend(
        &self,
        program: &KernelProgram,
        config: ReferenceConfig,
        backend: &impl ExpressionBackend,
    ) -> Result<Trajectory, Vec<Diagnostic>> {
        let mut observer = Uninterrupted;
        match self.run_controlled_with_expression_backend(
            program,
            config,
            backend,
            &mut observer,
        )? {
            ExecutionOutcome::Completed(trajectory) => Ok(trajectory),
            ExecutionOutcome::Cancelled(_) => {
                unreachable!("the uninterrupted observer cannot request cancellation")
            }
        }
    }

    fn run_controlled_with_expression_backend(
        &self,
        program: &KernelProgram,
        config: ReferenceConfig,
        backend: &impl ExpressionBackend,
        observer: &mut impl ExecutionObserver,
    ) -> Result<ExecutionOutcome, Vec<Diagnostic>> {
        if let Err(diagnostic) = config.validate() {
            return Err(vec![diagnostic]);
        }
        let mut plan = ExecutionPlan::new(program).map_err(|diagnostic| vec![diagnostic])?;
        let mut state = RuntimeState::new(program, &plan).map_err(|diagnostic| vec![diagnostic])?;

        solve_consistency(program, &plan, &mut state, 0.0, config, backend)
            .map_err(|diagnostic| vec![diagnostic])?;

        let mut time = 0.0;
        let mut steps = 0_usize;
        if plan.next_tick().is_some_and(RationalTime::is_zero) {
            execute_due_tick(program, &mut plan, &mut state, time, config, backend)
                .map_err(|diagnostic| vec![diagnostic])?;
            steps += 1;
        }

        let mut samples = Vec::new();
        let mut physical_samples = Vec::new();
        record_samples(
            program,
            &plan,
            &state,
            time,
            &mut samples,
            &mut physical_samples,
        );
        let mut last_event_time = None;
        let mut zero_time_events = 0_usize;

        let progress = accepted_progress(time, steps, config);
        if time < config.end_time
            && matches!(observer.observe(progress), ExecutionDirective::Cancel)
        {
            return Ok(ExecutionOutcome::Cancelled(progress));
        }

        while time < config.end_time {
            if steps >= config.max_steps {
                return Err(vec![config_error(format!(
                    "reference execution exceeded the {} step safety limit",
                    config.max_steps
                ))]);
            }

            let mut unconstrained_target = (time + config.max_step).min(config.end_time);
            let time_tolerance = 64.0
                * f64::EPSILON
                * time
                    .abs()
                    .max(config.end_time.abs())
                    .max(config.max_step)
                    .max(1.0);
            if (config.end_time - unconstrained_target).abs() <= time_tolerance {
                unconstrained_target = config.end_time;
            }
            let next_tick = plan.next_tick().filter(|tick| {
                tick.as_seconds_f64() <= config.end_time
                    && tick.as_seconds_f64() <= unconstrained_target + time_tolerance
            });
            let hits_tick = next_tick.is_some();
            let target = next_tick
                .map(RationalTime::as_seconds_f64)
                .unwrap_or(unconstrained_target)
                .max(time);

            if target > time {
                let start_state = state.clone();
                let mut trial_state = start_state.clone();
                solve_continuous_step(
                    program,
                    &plan,
                    &mut trial_state,
                    time,
                    target,
                    config,
                    backend,
                )
                .map_err(|diagnostic| vec![diagnostic])?;

                let crossings = crossing_events(
                    program,
                    &plan,
                    &start_state,
                    &trial_state,
                    time,
                    target,
                    config,
                    backend,
                )
                .map_err(|diagnostic| vec![diagnostic])?;
                if !crossings.is_empty() {
                    let mut located = Vec::with_capacity(crossings.len());
                    for crossing in crossings {
                        let event_time = locate_event_time(
                            program,
                            &plan,
                            &start_state,
                            &trial_state,
                            time,
                            target,
                            crossing,
                            config,
                            backend,
                        )
                        .map_err(|diagnostic| vec![diagnostic])?;
                        located.push((event_time, crossing));
                    }
                    located.sort_by(|left, right| {
                        left.0
                            .total_cmp(&right.0)
                            .then_with(|| left.1.cmp(&right.1))
                    });
                    let mut event_time = located[0].0;
                    if event::same_instant(event_time, target, config.event_time_tolerance) {
                        event_time = target;
                    }

                    let mut event_state = start_state.clone();
                    solve_continuous_step(
                        program,
                        &plan,
                        &mut event_state,
                        time,
                        event_time,
                        config,
                        backend,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    time = event_time;
                    state = event_state;
                    record_samples(
                        program,
                        &plan,
                        &state,
                        time,
                        &mut samples,
                        &mut physical_samples,
                    );

                    let mut relations = BTreeSet::new();
                    for (_, event_index) in located.iter().take_while(|(located_time, _)| {
                        event::same_instant(*located_time, event_time, config.event_time_tolerance)
                    }) {
                        relations.extend(&plan.events[*event_index].relations);
                    }
                    if hits_tick && event::same_instant(time, target, config.event_time_tolerance) {
                        let instant = next_tick.expect("hits_tick records an exact instant");
                        relations.extend(
                            plan.take_due_relations(instant)
                                .map_err(|diagnostic| vec![diagnostic])?,
                        );
                    }
                    execute_activated_relations(
                        program,
                        &plan,
                        &mut state,
                        time,
                        &relations,
                        "event-activation",
                        config,
                        backend,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    record_samples(
                        program,
                        &plan,
                        &state,
                        time,
                        &mut samples,
                        &mut physical_samples,
                    );

                    if last_event_time.is_some_and(|previous| {
                        event::same_instant(previous, time, config.event_time_tolerance)
                    }) {
                        zero_time_events += 1;
                    } else {
                        zero_time_events = 1;
                    }
                    last_event_time = Some(time);
                    if zero_time_events > config.max_zero_time_events {
                        return Err(vec![Diagnostic::error(
                            codes::INVALID_EXECUTION_CONFIG,
                            format!(
                                "event execution exceeded {} zero-time microsteps; possible Zeno behavior",
                                config.max_zero_time_events
                            ),
                        )
                        .with_graph_path(execution_path("event-activation", time))]);
                    }
                    steps += 1;
                    if time < config.end_time {
                        let progress = accepted_progress(time, steps, config);
                        if matches!(observer.observe(progress), ExecutionDirective::Cancel) {
                            return Ok(ExecutionOutcome::Cancelled(progress));
                        }
                    }
                    continue;
                }

                state = trial_state;
                time = target;
            } else if !hits_tick {
                return Err(vec![config_error(
                    "floating-point model time cannot advance by max_step",
                )]);
            }

            if hits_tick {
                execute_due_tick(program, &mut plan, &mut state, time, config, backend)
                    .map_err(|diagnostic| vec![diagnostic])?;
            }
            record_samples(
                program,
                &plan,
                &state,
                time,
                &mut samples,
                &mut physical_samples,
            );
            steps += 1;
            if time < config.end_time {
                let progress = accepted_progress(time, steps, config);
                if matches!(observer.observe(progress), ExecutionDirective::Cancel) {
                    return Ok(ExecutionOutcome::Cancelled(progress));
                }
            }
        }

        Ok(ExecutionOutcome::Completed(Trajectory::new(
            samples,
            physical_samples,
        )))
    }
}

#[derive(Debug)]
struct ExecutionPlan {
    continuous_relations: BTreeSet<RawId>,
    periodic: Vec<PeriodicTask>,
    events: Vec<EventTask>,
    differential_fields: BTreeSet<RawId>,
    algebraic_fields: BTreeSet<RawId>,
    discrete_fields: BTreeSet<RawId>,
    continuous_ports: BTreeSet<RawId>,
    signal_sources: BTreeMap<RawId, RawId>,
    physical_systems: Vec<ComposedResidualSystem>,
    physical_unknowns: Vec<PhysicalUnknown>,
    fields: BTreeSet<RawId>,
}

impl ExecutionPlan {
    fn new(program: &KernelProgram) -> Result<Self, Diagnostic> {
        let signal_sources = signal_sources(program)?;
        let physical_systems = physical_systems(program)?;
        let physical_unknowns = physical_systems
            .iter()
            .flat_map(|system| system.unknowns().iter().copied())
            .collect();
        let mut continuous_relations = BTreeSet::new();
        let mut periodic = Vec::new();
        let mut periodic_clocks = BTreeSet::new();
        let mut events = Vec::new();

        for node in program.nodes() {
            let KernelNode::Activation(activation) = node else {
                continue;
            };
            let activation_id = activation.id().erase();
            let relations = edge_targets(program, activation_id, eqiora_graph::EdgeKind::Activates);
            match activation.kind() {
                ActivationKind::Continuous => continuous_relations.extend(relations),
                ActivationKind::Periodic => {
                    let Some(clock_id) =
                        edge_targets(program, activation_id, eqiora_graph::EdgeKind::ClockedBy)
                            .into_iter()
                            .next()
                    else {
                        return Err(execution_error(
                            "periodic Activation has no validated ClockDomain",
                            0.0,
                        ));
                    };
                    let Some(KernelNode::ClockDomain(clock)) = program.node(clock_id) else {
                        return Err(execution_error(
                            "validated periodic ClockDomain definition is unavailable",
                            0.0,
                        ));
                    };
                    let ClockKind::Periodic { period, phase } = clock.kind() else {
                        return Err(execution_error(
                            "validated periodic clock changed kind",
                            0.0,
                        ));
                    };
                    periodic_clocks.insert(clock_id);
                    periodic.push(PeriodicTask {
                        relations,
                        period,
                        next: phase,
                    });
                }
                ActivationKind::Event { guard, direction } => {
                    events.push(EventTask {
                        activation: activation_id,
                        relations,
                        guard: guard.clone(),
                        direction: *direction,
                    });
                }
                ActivationKind::Guard { .. } => {
                    return Err(Diagnostic::error(
                        codes::NOT_IMPLEMENTED,
                        "guard activation follows the event foundation milestone",
                    )
                    .with_graph_path(kernel_path(activation_id)));
                }
                _ => {
                    return Err(Diagnostic::error(
                        codes::NOT_IMPLEMENTED,
                        "Activation kind is newer than this reference interpreter",
                    )
                    .with_graph_path(kernel_path(activation_id)));
                }
            }
        }

        events.sort_by_key(|event| event.activation);
        if !physical_systems.is_empty() && !events.is_empty() {
            return Err(Diagnostic::error(
                codes::NOT_IMPLEMENTED,
                "joint scalar physical execution does not yet compose zero-crossing events",
            )
            .with_graph_path(kernel_path(events[0].activation)));
        }
        if !physical_systems.is_empty()
            && let Some(second_clock) = periodic_clocks.iter().nth(1).copied()
        {
            return Err(Diagnostic::error(
                codes::NOT_IMPLEMENTED,
                "joint scalar physical execution admits at most one periodic ClockDomain",
            )
            .with_graph_path(kernel_path(second_clock)));
        }

        let mut discrete_fields = BTreeSet::new();
        let mut discrete_ports = BTreeSet::new();
        for relation in periodic
            .iter()
            .flat_map(|task| &task.relations)
            .chain(events.iter().flat_map(|task| &task.relations))
        {
            for symbol in relation_symbols(program, *relation)? {
                match symbol {
                    SymbolRef::Next(field) => {
                        discrete_fields.insert(field.erase());
                    }
                    SymbolRef::Port(port) if is_output_port(program, port.erase()) => {
                        discrete_ports.insert(port.erase());
                    }
                    _ => {}
                }
            }
        }

        let mut differential_fields = BTreeSet::new();
        let mut continuous_field_references = BTreeSet::new();
        let mut continuous_ports = BTreeSet::new();
        for &relation in &continuous_relations {
            for symbol in relation_symbols(program, relation)? {
                match symbol {
                    SymbolRef::Derivative(field) => {
                        differential_fields.insert(field.erase());
                    }
                    SymbolRef::Field(field) => {
                        continuous_field_references.insert(field.erase());
                    }
                    SymbolRef::Port(port) => {
                        let source = signal_sources
                            .get(&port.erase())
                            .copied()
                            .unwrap_or_else(|| port.erase());
                        if is_output_port(program, source) && !discrete_ports.contains(&source) {
                            continuous_ports.insert(source);
                        }
                    }
                    SymbolRef::Pre(_) | SymbolRef::Next(_) => {
                        return Err(Diagnostic::error(
                            codes::INVALID_KERNEL_DEFINITION,
                            "continuous Relations cannot read Pre or Next symbols",
                        )
                        .with_graph_path(kernel_path(relation)));
                    }
                    _ => {}
                }
            }
        }
        let algebraic_fields = continuous_field_references
            .difference(&differential_fields)
            .copied()
            .filter(|field| !discrete_fields.contains(field))
            .collect();
        let fields = program
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Field(field) => Some(field.id().erase()),
                _ => None,
            })
            .collect();

        Ok(Self {
            continuous_relations,
            periodic,
            events,
            differential_fields,
            algebraic_fields,
            discrete_fields,
            continuous_ports,
            signal_sources,
            physical_systems,
            physical_unknowns,
            fields,
        })
    }

    fn next_tick(&self) -> Option<RationalTime> {
        self.periodic.iter().map(|task| task.next).min()
    }

    fn take_due_relations(&mut self, instant: RationalTime) -> Result<BTreeSet<RawId>, Diagnostic> {
        let mut relations = BTreeSet::new();
        for task in self.periodic.iter_mut().filter(|task| task.next == instant) {
            relations.extend(&task.relations);
            task.next = task.next.checked_add(task.period)?;
        }
        Ok(relations)
    }
}

#[derive(Debug)]
struct PeriodicTask {
    relations: BTreeSet<RawId>,
    period: RationalTime,
    next: RationalTime,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    fields: BTreeMap<RawId, f64>,
    derivatives: BTreeMap<RawId, f64>,
    ports: BTreeMap<RawId, f64>,
    physical: BTreeMap<PhysicalUnknown, f64>,
}

impl RuntimeState {
    fn new(program: &KernelProgram, plan: &ExecutionPlan) -> Result<Self, Diagnostic> {
        let required = plan
            .differential_fields
            .union(&plan.discrete_fields)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut fields = BTreeMap::new();
        let mut ports = BTreeMap::new();
        for node in program.nodes() {
            match node {
                KernelNode::Field(field) => {
                    let id = field.id().erase();
                    match program.value(id) {
                        Some(value) => {
                            fields.insert(id, value.value());
                        }
                        None if required.contains(&id) => {
                            return Err(Diagnostic::error(
                                codes::MISSING_EXECUTION_INPUT,
                                format!("state Field {id} requires an initial value"),
                            )
                            .with_graph_path(kernel_path(id)));
                        }
                        None => {
                            fields.insert(id, 0.0);
                        }
                    }
                }
                KernelNode::Port(port)
                    if matches!(port.signal_contract(), Some((SignalDirection::Output, _))) =>
                {
                    ports.insert(port.id().erase(), 0.0);
                }
                _ => {}
            }
        }
        Ok(Self {
            fields,
            derivatives: BTreeMap::new(),
            ports,
            physical: plan
                .physical_unknowns
                .iter()
                .copied()
                .map(|unknown| (unknown, 0.0))
                .collect(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Variable {
    Field(RawId),
    Derivative(RawId),
    NextField(RawId),
    Port(RawId),
    Physical(PhysicalUnknown),
}

fn solve_consistency(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    state: &mut RuntimeState,
    time: f64,
    config: ReferenceConfig,
    backend: &impl ExpressionBackend,
) -> Result<(), Diagnostic> {
    let variables = plan
        .differential_fields
        .iter()
        .copied()
        .map(Variable::Derivative)
        .chain(plan.algebraic_fields.iter().copied().map(Variable::Field))
        .chain(plan.continuous_ports.iter().copied().map(Variable::Port))
        .chain(
            plan.physical_unknowns
                .iter()
                .copied()
                .map(Variable::Physical),
        )
        .collect::<Vec<_>>();
    let initial = variables
        .iter()
        .map(|variable| variable_value(*variable, state))
        .collect();
    let solution = solver::solve(
        initial,
        config.nonlinear_settings(),
        execution_path("consistency", time),
        |values| {
            let candidates = candidate_maps(&variables, values, state);
            evaluate_relations(
                program,
                &plan.continuous_relations,
                time,
                state,
                &candidates.fields,
                &candidates.derivatives,
                &BTreeMap::new(),
                &candidates.ports,
                &candidates.physical,
                &plan.signal_sources,
                &plan.physical_systems,
                backend,
            )
        },
    )?;
    commit_solution(&variables, &solution, state);
    Ok(())
}

fn solve_continuous_step(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    state: &mut RuntimeState,
    start: f64,
    end: f64,
    config: ReferenceConfig,
    backend: &impl ExpressionBackend,
) -> Result<(), Diagnostic> {
    let step = end - start;
    let variables = plan
        .differential_fields
        .iter()
        .chain(&plan.algebraic_fields)
        .copied()
        .map(Variable::Field)
        .chain(plan.continuous_ports.iter().copied().map(Variable::Port))
        .chain(
            plan.physical_unknowns
                .iter()
                .copied()
                .map(Variable::Physical),
        )
        .collect::<Vec<_>>();
    let initial = variables
        .iter()
        .map(|variable| match *variable {
            Variable::Field(id) if plan.differential_fields.contains(&id) => {
                state.fields[&id] + step * state.derivatives.get(&id).copied().unwrap_or(0.0)
            }
            _ => variable_value(*variable, state),
        })
        .collect();
    let solution = solver::solve(
        initial,
        config.nonlinear_settings(),
        execution_path("continuous-step", end),
        |values| {
            let mut candidates = candidate_maps(&variables, values, state);
            for &field in &plan.differential_fields {
                let candidate = candidates.fields[&field];
                candidates
                    .derivatives
                    .insert(field, (candidate - state.fields[&field]) / step);
            }
            evaluate_relations(
                program,
                &plan.continuous_relations,
                end,
                state,
                &candidates.fields,
                &candidates.derivatives,
                &BTreeMap::new(),
                &candidates.ports,
                &candidates.physical,
                &plan.signal_sources,
                &plan.physical_systems,
                backend,
            )
        },
    )?;
    let mut candidates = candidate_maps(&variables, &solution, state);
    for &field in &plan.differential_fields {
        candidates.derivatives.insert(
            field,
            (candidates.fields[&field] - state.fields[&field]) / step,
        );
    }
    commit_solution(&variables, &solution, state);
    state.derivatives.extend(candidates.derivatives);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn crossing_events(
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
fn locate_event_time(
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

fn execute_due_tick(
    program: &KernelProgram,
    plan: &mut ExecutionPlan,
    state: &mut RuntimeState,
    time: f64,
    config: ReferenceConfig,
    backend: &impl ExpressionBackend,
) -> Result<(), Diagnostic> {
    let Some(instant) = plan.next_tick() else {
        return Err(execution_error(
            "periodic execution requested with an empty calendar",
            time,
        ));
    };
    let relations = plan.take_due_relations(instant)?;
    execute_activated_relations(
        program,
        plan,
        state,
        time,
        &relations,
        "periodic-activation",
        config,
        backend,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_activated_relations(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    state: &mut RuntimeState,
    time: f64,
    relations: &BTreeSet<RawId>,
    phase: &str,
    config: ReferenceConfig,
    backend: &impl ExpressionBackend,
) -> Result<(), Diagnostic> {
    let mut variables = BTreeSet::new();
    for &relation in relations {
        for symbol in relation_symbols(program, relation)? {
            match symbol {
                SymbolRef::Next(field) => {
                    variables.insert(Variable::NextField(field.erase()));
                }
                SymbolRef::Port(port) if is_output_port(program, port.erase()) => {
                    variables.insert(Variable::Port(port.erase()));
                }
                _ => {}
            }
        }
    }
    let variables = variables.into_iter().collect::<Vec<_>>();
    let initial = variables
        .iter()
        .map(|variable| variable_value(*variable, state))
        .collect();
    let solution = solver::solve(
        initial,
        config.nonlinear_settings(),
        execution_path(phase, time),
        |values| {
            let candidates = candidate_maps(&variables, values, state);
            evaluate_relations(
                program,
                relations,
                time,
                state,
                &candidates.fields,
                &candidates.derivatives,
                &candidates.next_fields,
                &candidates.ports,
                &candidates.physical,
                &plan.signal_sources,
                &[],
                backend,
            )
        },
    )?;
    commit_solution(&variables, &solution, state);
    solve_consistency(program, plan, state, time, config, backend)
}

struct CandidateMaps {
    fields: BTreeMap<RawId, f64>,
    derivatives: BTreeMap<RawId, f64>,
    next_fields: BTreeMap<RawId, f64>,
    ports: BTreeMap<RawId, f64>,
    physical: BTreeMap<PhysicalUnknown, f64>,
}

fn candidate_maps(variables: &[Variable], values: &[f64], state: &RuntimeState) -> CandidateMaps {
    let mut candidates = CandidateMaps {
        fields: BTreeMap::new(),
        derivatives: state.derivatives.clone(),
        next_fields: BTreeMap::new(),
        ports: BTreeMap::new(),
        physical: BTreeMap::new(),
    };
    for (variable, value) in variables.iter().zip(values) {
        match *variable {
            Variable::Field(id) => {
                candidates.fields.insert(id, *value);
            }
            Variable::Derivative(id) => {
                candidates.derivatives.insert(id, *value);
            }
            Variable::NextField(id) => {
                candidates.next_fields.insert(id, *value);
            }
            Variable::Port(id) => {
                candidates.ports.insert(id, *value);
            }
            Variable::Physical(unknown) => {
                candidates.physical.insert(unknown, *value);
            }
        }
    }
    candidates
}

fn commit_solution(variables: &[Variable], values: &[f64], state: &mut RuntimeState) {
    for (variable, value) in variables.iter().zip(values) {
        match *variable {
            Variable::Field(id) | Variable::NextField(id) => {
                state.fields.insert(id, *value);
            }
            Variable::Derivative(id) => {
                state.derivatives.insert(id, *value);
            }
            Variable::Port(id) => {
                state.ports.insert(id, *value);
            }
            Variable::Physical(unknown) => {
                state.physical.insert(unknown, *value);
            }
        }
    }
}

fn variable_value(variable: Variable, state: &RuntimeState) -> f64 {
    match variable {
        Variable::Field(id) | Variable::NextField(id) => state.fields[&id],
        Variable::Derivative(id) => state.derivatives.get(&id).copied().unwrap_or(0.0),
        Variable::Port(id) => state.ports.get(&id).copied().unwrap_or(0.0),
        Variable::Physical(unknown) => state.physical.get(&unknown).copied().unwrap_or(0.0),
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_relations(
    program: &KernelProgram,
    relations: &BTreeSet<RawId>,
    time: f64,
    state: &RuntimeState,
    field_candidates: &BTreeMap<RawId, f64>,
    derivatives: &BTreeMap<RawId, f64>,
    next_fields: &BTreeMap<RawId, f64>,
    port_candidates: &BTreeMap<RawId, f64>,
    physical_candidates: &BTreeMap<PhysicalUnknown, f64>,
    signal_sources: &BTreeMap<RawId, RawId>,
    physical_systems: &[ComposedResidualSystem],
    backend: &impl ExpressionBackend,
) -> Result<Vec<f64>, Diagnostic> {
    let context = EvalContext {
        program,
        time,
        fields: &state.fields,
        field_candidates,
        derivatives,
        next_fields,
        ports: &state.ports,
        port_candidates,
        signal_sources,
        physical: &state.physical,
        physical_candidates,
    };
    let mut residuals = Vec::new();
    for &relation in relations {
        let Some(KernelNode::Relation(definition)) = program.node(relation) else {
            return Err(execution_error(
                "validated Relation definition is unavailable",
                time,
            ));
        };
        residuals.extend(
            backend.evaluate(relation, definition.residuals(), &mut |symbol| {
                evaluate::resolve_symbol(symbol, &context)
            })?,
        );
    }
    for system in physical_systems {
        for junction in system.junctions() {
            residuals.extend(backend.evaluate(
                junction.connection().erase(),
                junction.dag(),
                &mut |symbol| evaluate::resolve_symbol(symbol, &context),
            )?);
        }
    }
    Ok(residuals)
}

fn relation_symbols(
    program: &KernelProgram,
    relation: RawId,
) -> Result<Vec<SymbolRef>, Diagnostic> {
    let Some(KernelNode::Relation(definition)) = program.node(relation) else {
        return Err(execution_error(
            "validated Relation definition is unavailable",
            0.0,
        ));
    };
    Ok(definition
        .residuals()
        .nodes()
        .iter()
        .filter_map(|node| match node {
            ExprNode::Symbol(symbol) => Some(*symbol),
            _ => None,
        })
        .collect())
}

fn physical_systems(program: &KernelProgram) -> Result<Vec<ComposedResidualSystem>, Diagnostic> {
    let mut systems = BTreeMap::new();
    for node in program.nodes() {
        let KernelNode::Connection(connection) = node else {
            continue;
        };
        if connection.semantics() != ConnectionSemantics::Conserving {
            continue;
        }
        let connection_id = connection.id();
        let members = edge_targets(
            program,
            connection_id.erase(),
            eqiora_graph::EdgeKind::Connects,
        );
        let scalar_physical = members.iter().all(|member| {
            matches!(
                program.node(*member),
                Some(KernelNode::Port(port)) if port.physical_domain().is_some()
            )
        });
        if !scalar_physical {
            return Err(Diagnostic::error(
                codes::NOT_IMPLEMENTED,
                "reference execution does not assign physical meaning to legacy conserving markers",
            )
            .with_graph_path(kernel_path(connection_id.erase())));
        }
        let system = program.compose_scalar_physical_execution_subsystem(connection_id)?;
        systems
            .entry(system.subsystem().connection().erase())
            .or_insert(system);
    }
    Ok(systems.into_values().collect())
}

fn signal_sources(program: &KernelProgram) -> Result<BTreeMap<RawId, RawId>, Diagnostic> {
    let mut sources = BTreeMap::new();
    for node in program.nodes() {
        let KernelNode::Connection(connection) = node else {
            continue;
        };
        let id = connection.id().erase();
        match connection.semantics() {
            ConnectionSemantics::Signal => {
                let ports = edge_targets(program, id, eqiora_graph::EdgeKind::Connects);
                let Some(output) = ports
                    .iter()
                    .find(|port| is_output_port(program, **port))
                    .copied()
                else {
                    return Err(execution_error(
                        "signal Connection has no validated output Port",
                        0.0,
                    ));
                };
                for input in ports.into_iter().filter(|port| *port != output) {
                    sources.insert(input, output);
                }
            }
            ConnectionSemantics::Conserving | ConnectionSemantics::SpatialPeriodic => {}
            _ => {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "Connection semantics are newer than this reference interpreter",
                )
                .with_graph_path(kernel_path(id)));
            }
        }
    }
    Ok(sources)
}

fn is_output_port(program: &KernelProgram, port: RawId) -> bool {
    matches!(
        program.node(port),
        Some(KernelNode::Port(definition))
            if matches!(
                definition.signal_contract(),
                Some((SignalDirection::Output, _))
            )
    )
}

fn edge_targets(
    program: &KernelProgram,
    from: RawId,
    kind: eqiora_graph::EdgeKind,
) -> BTreeSet<RawId> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.from() == from && edge.kind() == kind)
        .map(eqiora_graph::Edge::to)
        .collect()
}

fn record_samples(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    state: &RuntimeState,
    time: f64,
    samples: &mut Vec<Sample>,
    physical_samples: &mut Vec<PhysicalSample>,
) {
    for &field in &plan.fields {
        let Some(KernelNode::Field(definition)) = program.node(field) else {
            continue;
        };
        samples.push(Sample::new(
            time,
            field,
            DynQuantity::new(state.fields[&field], definition.dimension()),
        ));
    }
    for (&unknown, &value) in &state.physical {
        let Some(KernelNode::Port(port)) = program.node(unknown.port().erase()) else {
            continue;
        };
        let Some(domain) = port.physical_domain() else {
            continue;
        };
        let Some(KernelNode::Domain(domain)) = program.node(domain.erase()) else {
            continue;
        };
        let DomainKind::ScalarPhysical {
            across_dimension,
            through_dimension,
        } = domain.kind()
        else {
            continue;
        };
        let dimension = match unknown {
            PhysicalUnknown::Across(_) => *across_dimension,
            PhysicalUnknown::Through(_) => *through_dimension,
        };
        physical_samples.push(PhysicalSample::new(
            time,
            unknown,
            DynQuantity::new(value, dimension),
        ));
    }
}

fn config_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_EXECUTION_CONFIG, message)
        .with_graph_path(GraphPath::new(["execution", "reference-config"]))
}

fn execution_error(message: impl Into<String>, time: f64) -> Diagnostic {
    Diagnostic::error(codes::INVALID_KERNEL_DEFINITION, message)
        .with_graph_path(execution_path("reference", time))
}

fn execution_path(phase: &str, time: f64) -> GraphPath {
    GraphPath::new([
        "execution".to_owned(),
        phase.to_owned(),
        format!("t={time:.17}"),
    ])
}

fn kernel_path(id: RawId) -> GraphPath {
    GraphPath::new(["semantic", &format!("{:?}", id.kind()), &id.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_config_rejects_non_advancing_steps() {
        let diagnostic = ReferenceConfig::new(1.0, 0.0).expect_err("zero step");

        assert_eq!(diagnostic.code(), codes::INVALID_EXECUTION_CONFIG);
    }
}
