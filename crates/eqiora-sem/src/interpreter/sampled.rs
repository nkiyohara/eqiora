//! Bounded sampled inputs and exact in-process accepted-state restart.
use super::*;
use eqiora_core::ValueLiteral;

const MAX_SAMPLED_VALUES: usize = super::direct_assignments::MAX_COMPONENTS;

#[derive(Debug, Clone)]
struct InputTable {
    clock: RawId,
    values: Vec<ValueLiteral>,
    cursor: usize,
}

/// A bounded reference run with complete external tick inputs.
/// The immutable program, exact calendar, and accepted values remain together.
/// Complete input tables and retained output samples each have a one-million-scalar-component cap.
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
        validate_output_budget(program, config, MAX_SAMPLED_VALUES)?;
        let mut inputs = BTreeMap::new();
        let mut sample_count = 0usize;
        for (port, clock, values) in supplied {
            sample_count = add_sample_count(sample_count, values.len(), MAX_SAMPLED_VALUES)?;
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
            // Count complete components before scanning or cloning any supplied values.
            let components = value_type
                .shape()
                .component_count()
                .ok_or_else(|| config_error("sampled input component count exceeds bounds"))?;
            let extra = values
                .len()
                .checked_mul(components.saturating_sub(1))
                .ok_or_else(|| config_error("sampled input component count exceeds bounds"))?;
            sample_count = add_sample_count(sample_count, extra, MAX_SAMPLED_VALUES)?;
            if values.iter().any(|v| {
                v.value_type() != value_type || !direct_assignments::supported_type(v.value_type())
            }) {
                return Err(config_error(
                    "sampled input values must match the complete supported Port type",
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

    /// Accepted memory or continuous algebraic value. A clocked Variable is present
    /// only after its own accepted tick, until execution advances to another instant.
    #[must_use]
    pub fn field(&self, field: RawId) -> Option<ValueLiteral> {
        let KernelNode::Field(definition) = self.program.node(field)? else {
            return None;
        };
        self.state.typed_fields.get(&field).cloned().or_else(|| {
            self.state.fields.get(&field).and_then(|value| {
                ValueLiteral::from_real(definition.value_type().clone(), *value).ok()
            })
        })
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
        candidate
            .typed_ports
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
                if let Some(real) = value.real_scalar_value() {
                    candidate.ports.insert(port, real.value());
                } else {
                    candidate.typed_ports.insert(port, value.clone());
                }
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
            let value = if let Some(value) = candidate.typed_ports.get(&source) {
                value.clone()
            } else {
                let value = candidate
                    .ports
                    .get(&source)
                    .ok_or_else(|| config_error("requested output has no accepted tick value"))?;
                ValueLiteral::from_real(value_type.clone(), *value)
                    .map_err(|_| config_error("sampled output is not a finite real scalar"))?
            };
            if value.value_type() != value_type {
                return Err(config_error(
                    "sampled output differs from its complete Port type",
                ));
            }
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

fn add_sample_count(current: usize, additional: usize, limit: usize) -> Result<usize, Diagnostic> {
    current
        .checked_add(additional)
        .filter(|total| *total <= limit)
        .ok_or_else(|| config_error("sampled values exceed the retention budget"))
}

fn validate_output_budget(
    program: &KernelProgram,
    config: ReferenceConfig,
    limit: usize,
) -> Result<(), Diagnostic> {
    let mut count = 0;
    let mut ticks_by_clock = BTreeMap::new();
    for &port in program.boundary() {
        if !matches!(program.node(port), Some(KernelNode::Port(definition)) if matches!(definition.signal_contract(), Some((SignalDirection::Output, _))))
        {
            continue;
        }
        let Some(clock) = port_clock(program, port)? else {
            continue;
        };
        let ticks = match ticks_by_clock.entry(clock) {
            std::collections::btree_map::Entry::Occupied(entry) => *entry.get(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                *entry.insert(required_ticks(program, clock, config)?)
            }
        };
        let Some(KernelNode::Port(definition)) = program.node(port) else {
            unreachable!()
        };
        let (_, value_type) = definition.signal_contract().expect("output signal");
        let components = value_type
            .shape()
            .component_count()
            .and_then(|components| components.checked_mul(ticks))
            .ok_or_else(|| config_error("sampled output component count exceeds bounds"))?;
        count = add_sample_count(count, components, limit)?;
    }
    Ok(())
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
        return Err(config_error("sampled clock is not a ClockDomain"));
    };
    let ClockKind::Periodic { period, phase } = definition.kind() else {
        return Err(config_error("sampled clock is not periodic"));
    };
    let mut next = phase;
    let mut count = 0;
    while within_horizon(next, config.end_time) {
        if count >= config.max_steps || count >= MAX_SAMPLED_VALUES {
            return Err(config_error("sampled calendar exceeds step budget"));
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
    fn retention_budget_counts_each_output_on_a_shared_clock_before_initialization() {
        use eqiora_core::{Id, OntologyId, ScalarDomain, ValueType, entity::kinds};
        use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
        use eqiora_schema::kernel::{ClockDomainDef, RelationDef};
        use eqiora_schema::{Model, ModelView};
        for components in [1, 3] {
            let model = OntologyId::<Model>::new();
            let clock = Id::<kinds::ClockDomain>::new();
            let outputs = [Id::<kinds::Port>::new(), Id::new()];
            let field = Id::<kinds::Field>::new();
            let relation = Id::<kinds::Relation>::new();
            let ty =
                ValueType::scalar(ScalarDomain::Real, eqiora_core::DimExponents::DIMENSIONLESS);
            let ty = if components == 1 {
                ty
            } else {
                ty.array(3).unwrap()
            };
            let mut dag = eqiora_schema::kernel::ExprDagBuilder::new();
            let value = dag.symbol(SymbolRef::Field(field)).unwrap();
            let zero = dag
                .constant(ValueLiteral::from_real(ty.clone(), 0.0).unwrap())
                .unwrap();
            let nodes = [
                KernelNode::from(
                    ClockDomainDef::periodic(
                        clock,
                        RationalTime::new(1, 1).unwrap(),
                        RationalTime::ZERO,
                    )
                    .unwrap(),
                ),
                eqiora_schema::kernel::FieldDef::new(
                    field,
                    ty.clone(),
                    eqiora_schema::kernel::FieldRole::State,
                )
                .into(),
                RelationDef::initial(relation, dag.finish([value, zero]).unwrap())
                    .unwrap()
                    .into(),
                eqiora_schema::kernel::PortDef::signal(
                    outputs[0],
                    SignalDirection::Output,
                    ty.clone(),
                )
                .into(),
                eqiora_schema::kernel::PortDef::signal(outputs[1], SignalDirection::Output, ty)
                    .into(),
            ];
            let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
            let mut transaction = Transaction::new("shared output retention bound");
            for node in nodes {
                transaction.push(Op::DefineKernelNode { node });
            }
            transaction.push(Op::Connect {
                from: relation.erase(),
                to: field.erase(),
                edge: EdgeKind::DependsOn,
            });
            for output in outputs {
                transaction.push(Op::Connect {
                    from: output.erase(),
                    to: clock.erase(),
                    edge: EdgeKind::ClockedBy,
                });
            }
            transaction.push(Op::DefineOntologyView {
                view: ModelView::new(model, members, outputs.map(Id::erase))
                    .unwrap()
                    .into(),
            });
            let mut store = InMemoryGraphStore::new();
            store.commit(transaction).unwrap();
            let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
            let config = ReferenceConfig::new(1., 1.).unwrap();
            // Two outputs at t=0 and t=1, each retaining one or three components.
            assert!(validate_output_budget(&program, config, 4 * components).is_ok());
            assert!(validate_output_budget(&program, config, 4 * components - 1).is_err());
            assert!(add_sample_count(usize::MAX, 1, usize::MAX).is_err());
        }
    }

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
