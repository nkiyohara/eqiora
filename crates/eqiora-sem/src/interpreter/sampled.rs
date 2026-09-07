//! Bounded sampled inputs and exact in-process accepted-state restart.
use super::*;
use eqiora_core::ValueLiteral;

const MAX_INPUT_SAMPLES: usize = 1_000_000;

#[derive(Debug, Clone)]
struct InputTable {
    clock: RawId,
    values: Vec<ValueLiteral>,
    cursor: usize,
}

/// A bounded reference run with complete external tick inputs.
/// The immutable program, exact calendar, and accepted values remain together.
#[derive(Debug, Clone)]
pub struct SampledSession {
    program: KernelProgram,
    config: ReferenceConfig,
    plan: ExecutionPlan,
    state: RuntimeState,
    inputs: BTreeMap<RawId, InputTable>,
    outputs: BTreeMap<(RawId, u64), (RationalTime, ValueLiteral)>,
    time: f64,
    accepted_steps: usize,
}

impl Interpreter {
    /// Initialize before tick zero using complete, exact-clock input tables.
    /// Values are indexed from each clock's first tick, including a delayed phase.
    /// Missing, duplicate, foreign, mistyped, or incompletely covered inputs reject.
    pub fn sampled_session(
        &self,
        program: &KernelProgram,
        config: ReferenceConfig,
        inputs: impl IntoIterator<Item = (RawId, RawId, Vec<ValueLiteral>)>,
    ) -> Result<SampledSession, Vec<Diagnostic>> {
        SampledSession::fresh(program, config, inputs).map_err(|error| vec![error])
    }

    /// Resume exactly the accepted checkpoint without running initial equations.
    /// A different program, including a different value revision, rejects.
    pub fn resume_sampled(
        &self,
        program: &KernelProgram,
        checkpoint: &SampledSession,
    ) -> Result<SampledSession, Vec<Diagnostic>> {
        if program != &checkpoint.program {
            return Err(vec![config_error(
                "sampled checkpoint belongs to a different exact program",
            )]);
        }
        Ok(checkpoint.clone())
    }
}

impl SampledSession {
    fn fresh(
        program: &KernelProgram,
        config: ReferenceConfig,
        supplied: impl IntoIterator<Item = (RawId, RawId, Vec<ValueLiteral>)>,
    ) -> Result<Self, Diagnostic> {
        config.validate()?;
        let mut plan = ExecutionPlan::new(program)?;
        // Exposed clocks also tick for pure forwarding without an equation activation.
        for &port in program.boundary() {
            if let Some(clock) = port_clock(program, port)?
                && !plan.periodic.iter().any(|task| task.clock == clock)
            {
                let Some(KernelNode::ClockDomain(definition)) = program.node(clock) else {
                    return Err(config_error("boundary clock is not a ClockDomain"));
                };
                let ClockKind::Periodic { period, phase } = definition.kind() else {
                    return Err(config_error(
                        "sampled boundary requires a periodic ClockDomain",
                    ));
                };
                plan.periodic.push(PeriodicTask {
                    clock,
                    tick_index: 0,
                    relations: BTreeSet::new(),
                    period,
                    next: phase,
                });
            }
        }
        if !plan.events.is_empty() {
            return Err(config_error(
                "sampled sessions do not yet checkpoint event localization",
            ));
        }
        let required = program
            .boundary()
            .iter()
            .filter_map(|id| match program.node(*id) {
                Some(KernelNode::Port(port))
                    if matches!(port.signal_contract(), Some((SignalDirection::Input, _))) =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let mut inputs = BTreeMap::new();
        let mut sample_count = 0usize;
        for (port, clock, values) in supplied {
            if !required.contains(&port) || inputs.contains_key(&port) {
                return Err(config_error(
                    "sampled input is duplicate or is not an exposed input occurrence",
                ));
            }
            if port_clock(program, port)? != Some(clock) {
                return Err(config_error(
                    "sampled input requires its exact declared ClockDomain",
                ));
            }
            let Some(KernelNode::Port(definition)) = program.node(port) else {
                unreachable!()
            };
            let (_, value_type) = definition.signal_contract().expect("required signal input");
            if values
                .iter()
                .any(|v| v.value_type() != value_type || v.real_scalar_value().is_none())
            {
                return Err(config_error(
                    "sampled input values must match the complete real scalar Port type",
                ));
            }
            sample_count = sample_count
                .checked_add(values.len())
                .ok_or_else(|| config_error("sampled input count overflow"))?;
            if sample_count > MAX_INPUT_SAMPLES {
                return Err(config_error(
                    "sampled input tables exceed one million values",
                ));
            }
            if values.len() != required_ticks(program, clock, config)? {
                return Err(config_error(
                    "sampled input table must cover every requested tick exactly",
                ));
            }
            inputs.insert(
                port,
                InputTable {
                    clock,
                    values,
                    cursor: 0,
                },
            );
        }
        if inputs.keys().copied().collect::<BTreeSet<_>>() != required {
            return Err(config_error(
                "sampled run is missing an exposed input table",
            ));
        }
        let mut state = RuntimeState::new(program, &plan)?;
        solve_initialization(
            program,
            &plan,
            &mut state,
            config,
            &ReferenceExpressionBackend,
        )?;
        // A clocked sample has no value before its first accepted tick.
        state
            .ports
            .retain(|port, _| port_clock(program, *port).ok().flatten().is_none());
        Ok(Self {
            program: program.clone(),
            config,
            plan,
            state,
            inputs,
            outputs: BTreeMap::new(),
            time: 0.0,
            accepted_steps: 0,
        })
    }

    /// Capture all accepted state, including the request, input tables and cursors.
    /// Resume cannot replace future inputs. This in-process snapshot has no
    /// serialized checkpoint-format promise.
    #[must_use]
    pub fn checkpoint(&self) -> Self {
        self.clone()
    }

    /// Advance at most `count` distinct exact tick instants; return the accepted count.
    /// A failed boundary changes neither values, clock progress, inputs nor outputs.
    pub fn advance_ticks(&mut self, count: usize) -> Result<usize, Vec<Diagnostic>> {
        let mut accepted = 0;
        while accepted < count && self.next_tick().is_some() {
            self.advance_one().map_err(|error| vec![error])?;
            accepted += 1;
        }
        Ok(accepted)
    }

    /// Next requested exact instant, absent after the inclusive run horizon.
    #[must_use]
    pub fn next_tick(&self) -> Option<RationalTime> {
        self.plan
            .next_tick()
            .filter(|tick| within_horizon(*tick, self.config.end_time))
    }

    /// Accepted initialized memory or algebraic Field value.
    #[must_use]
    pub fn field(&self, field: RawId) -> Option<DynQuantity> {
        let KernelNode::Field(definition) = self.program.node(field)? else {
            return None;
        };
        self.state
            .fields
            .get(&field)
            .map(|value| DynQuantity::new(*value, definition.dimension()))
    }

    /// An exposed output's accepted sample at its own zero-based tick index.
    /// Absent before that tick; this never manufactures a held signal value.
    #[must_use]
    pub fn output(&self, port: RawId, tick_index: u64) -> Option<(RationalTime, &ValueLiteral)> {
        self.outputs
            .get(&(port, tick_index))
            .map(|(time, value)| (*time, value))
    }

    fn advance_one(&mut self) -> Result<(), Diagnostic> {
        let instant = self.next_tick().expect("caller checked the next tick");
        let time = instant.as_seconds_f64();
        let mut candidate = self.state.clone();
        let mut plan = self.plan.clone();
        let mut steps = self.accepted_steps;
        let mut current_time = self.time;
        while current_time < time {
            let end = (current_time + self.config.max_step).min(time);
            if end <= current_time || steps >= self.config.max_steps {
                return Err(config_error("sampled execution exhausted its step budget"));
            }
            solve_continuous_step(
                &self.program,
                &plan,
                &mut candidate,
                current_time,
                end,
                self.config,
                &ReferenceExpressionBackend,
            )?;
            current_time = end;
            steps += 1;
        }
        if steps >= self.config.max_steps {
            return Err(config_error("sampled execution exhausted its step budget"));
        }
        let due = plan
            .periodic
            .iter()
            .filter(|task| task.next == instant)
            .map(|task| (task.clock, task.tick_index))
            .collect::<BTreeMap<_, _>>();
        candidate
            .ports
            .retain(|port, _| port_clock(&self.program, *port).ok().flatten().is_none());
        for (&port, input) in &self.inputs {
            if let Some(index) = due.get(&input.clock) {
                if *index != input.cursor as u64 {
                    return Err(config_error(
                        "sampled input cursor differs from its exact calendar",
                    ));
                }
                let value = input
                    .values
                    .get(input.cursor)
                    .ok_or_else(|| config_error("sampled input coverage exhausted"))?;
                candidate.ports.insert(
                    port,
                    value
                        .real_scalar_value()
                        .expect("admitted real scalar")
                        .value(),
                );
            }
        }
        execute_due_tick(
            &self.program,
            &mut plan,
            &mut candidate,
            time,
            self.config,
            &ReferenceExpressionBackend,
        )?;
        let mut outputs = Vec::new();
        for &port in self.program.boundary() {
            let Some(KernelNode::Port(definition)) = self.program.node(port) else {
                continue;
            };
            let Some((SignalDirection::Output, value_type)) = definition.signal_contract() else {
                continue;
            };
            let Some(clock) = port_clock(&self.program, port)? else {
                continue;
            };
            let Some(index) = due.get(&clock) else {
                continue;
            };
            let source = plan.signal_sources.get(&port).copied().unwrap_or(port);
            let value = candidate
                .ports
                .get(&source)
                .ok_or_else(|| config_error("requested output has no accepted tick value"))?;
            let value = ValueLiteral::from_real(value_type.clone(), *value)
                .map_err(|_| config_error("sampled output is not a finite real scalar"))?;
            outputs.push(((port, *index), (instant, value)));
        }
        self.plan = plan;
        self.state = candidate;
        self.time = time;
        self.accepted_steps = steps + 1;
        for input in self
            .inputs
            .values_mut()
            .filter(|input| due.contains_key(&input.clock))
        {
            input.cursor += 1;
        }
        self.outputs.extend(outputs);
        Ok(())
    }
}

fn port_clock(program: &KernelProgram, port: RawId) -> Result<Option<RawId>, Diagnostic> {
    let clocks = edge_targets(program, port, eqiora_graph::EdgeKind::ClockedBy);
    if clocks.len() > 1 {
        return Err(config_error("Port has ambiguous activation"));
    }
    Ok(clocks.first().copied())
}

fn required_ticks(
    program: &KernelProgram,
    clock: RawId,
    config: ReferenceConfig,
) -> Result<usize, Diagnostic> {
    let Some(KernelNode::ClockDomain(definition)) = program.node(clock) else {
        return Err(config_error("input clock is not a ClockDomain"));
    };
    let ClockKind::Periodic { period, phase } = definition.kind() else {
        return Err(config_error("input clock is not periodic"));
    };
    let mut next = phase;
    let mut count = 0;
    while within_horizon(next, config.end_time) {
        if count >= config.max_steps || count >= MAX_INPUT_SAMPLES {
            return Err(config_error("sampled input calendar exceeds step budget"));
        }
        count += 1;
        next = next.checked_add(period)?;
    }
    Ok(count)
}

// Compare n/d <= significand * 2^exponent without rounding either operand.
// Configuration admission guarantees a finite, non-negative binary64 horizon.
pub(super) fn within_horizon(tick: RationalTime, horizon: f64) -> bool {
    debug_assert!(horizon.is_finite() && horizon >= 0.0);
    if tick.is_zero() {
        return true;
    }
    let bits = horizon.to_bits();
    let biased_exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if biased_exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1_u64 << 52), biased_exponent - 1023 - 52)
    };
    let numerator = u128::from(tick.numerator());
    // At most 64 + 53 bits, so this product always fits.
    let scaled_horizon = u128::from(tick.denominator()) * u128::from(significand);
    if exponent >= 0 {
        let shift = exponent as u32;
        if shift >= 128 || scaled_horizon > (u128::MAX >> shift) {
            return true;
        }
        numerator <= scaled_horizon << shift
    } else {
        let shift = (-exponent) as u32;
        if shift >= 128 || numerator > (u128::MAX >> shift) {
            return false;
        }
        numerator << shift <= scaled_horizon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_horizon_handles_binary64_extremes_without_large_integer_allocation() {
        let smallest_tick = RationalTime::new(1, u64::MAX).unwrap();
        assert!(within_horizon(RationalTime::ZERO, 0.));
        assert!(!within_horizon(smallest_tick, 0.));
        assert!(!within_horizon(smallest_tick, f64::from_bits(1)));
        assert!(!within_horizon(smallest_tick, f64::MIN_POSITIVE));
        assert!(within_horizon(
            RationalTime::new(u64::MAX, 1).unwrap(),
            f64::MAX
        ));
        assert!(within_horizon(RationalTime::new(1, 2).unwrap(), 0.5));
        assert!(!within_horizon(
            RationalTime::new(1, 2).unwrap(),
            f64::from_bits(0.5_f64.to_bits() - 1)
        ));
    }
}
