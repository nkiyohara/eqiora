use eqiora_core::entity::kinds;
use eqiora_core::{
    DimExponents, DynQuantity, Id, OntologyId, RawId, ScalarDomain, ValueLiteral, ValueType,
};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ClockDomainDef, ConnectionDef, ConnectionSemantics, ExprDag, ExprDagBuilder,
    ExprNode, FieldDef, FieldRole, KernelNode, PortDef, RationalTime, RelationDef, SignalDirection,
    SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};

struct Fixture {
    program: KernelProgram,
    clock: RawId,
    inputs: [RawId; 2],
    outputs: [RawId; 3],
    fields: [RawId; 3],
}
fn voltage() -> DimExponents {
    DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).unwrap()
}
fn ty(dimension: DimExponents) -> ValueType {
    ValueType::scalar(ScalarDomain::Real, dimension)
}
fn connect(t: &mut Transaction, from: RawId, to: RawId, edge: EdgeKind) {
    t.push(Op::Connect { from, to, edge });
}
fn relation(
    t: &mut Transaction,
    nodes: &mut Vec<KernelNode>,
    dag: ExprDag,
    initial: bool,
    activation: Id<kinds::Activation>,
) {
    let id = Id::<kinds::Relation>::new();
    for node in dag.nodes() {
        let target = match node {
            ExprNode::Symbol(SymbolRef::Field(f) | SymbolRef::Pre(f) | SymbolRef::Next(f)) => {
                Some(f.erase())
            }
            ExprNode::Symbol(SymbolRef::Port(p)) => Some(p.erase()),
            _ => None,
        };
        if let Some(target) = target {
            connect(t, id.erase(), target, EdgeKind::DependsOn);
        }
    }
    nodes.push(
        if initial {
            RelationDef::initial(id, dag).unwrap()
        } else {
            RelationDef::new(id, dag).unwrap()
        }
        .into(),
    );
    if !initial {
        connect(t, activation.erase(), id.erase(), EdgeKind::Activates);
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Failure {
    None,
    Tick,
    Consistency,
}

fn fixture(period: u64, phase: u64, reverse: bool, failure: Failure) -> Fixture {
    let mut t = Transaction::new("three independent sampled memories");
    let model = OntologyId::<Model>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let activation = Id::<kinds::Activation>::new();
    let inputs = [Id::<kinds::Port>::new(), Id::new()];
    let outputs = [Id::<kinds::Port>::new(), Id::new(), Id::new()];
    let fields = [Id::<kinds::Field>::new(), Id::new(), Id::new()];
    let rate = voltage()
        .div(DimExponents::from_integers([0, 0, 1, 0, 0, 0, 0]).unwrap())
        .unwrap();
    let mut nodes = vec![
        ClockDomainDef::periodic(
            clock,
            RationalTime::new(period, 1000).unwrap(),
            RationalTime::new(phase, 1000).unwrap(),
        )
        .unwrap()
        .into(),
        ActivationDef::periodic(activation).into(),
    ];
    connect(
        &mut t,
        activation.erase(),
        clock.erase(),
        EdgeKind::ClockedBy,
    );
    for (i, input) in inputs.iter().enumerate() {
        nodes.push(
            PortDef::signal(
                *input,
                SignalDirection::Input,
                ty(if i == 0 { voltage() } else { rate }),
            )
            .into(),
        );
        connect(&mut t, input.erase(), clock.erase(), EdgeKind::ClockedBy);
    }
    let mut order = vec![0, 1, 2];
    if reverse {
        order.reverse();
    }
    for i in order {
        nodes.push(FieldDef::new(fields[i], ty(voltage()), FieldRole::State).into());
        connect(
            &mut t,
            fields[i].erase(),
            clock.erase(),
            EdgeKind::ClockedBy,
        );
        let child_in = Id::<kinds::Port>::new();
        let child_out = Id::<kinds::Port>::new();
        for (port, direction, dimension) in [
            (
                child_in,
                SignalDirection::Input,
                if i == 2 { rate } else { voltage() },
            ),
            (child_out, SignalDirection::Output, voltage()),
            (outputs[i], SignalDirection::Output, voltage()),
        ] {
            nodes.push(PortDef::signal(port, direction, ty(dimension)).into());
            connect(&mut t, port.erase(), clock.erase(), EdgeKind::ClockedBy);
        }
        // Preserve each wrapper endpoint while forwarding both input and output.
        for (source, sink, direction, dimension) in [
            (
                inputs[usize::from(i == 2)],
                child_in,
                SignalDirection::Input,
                if i == 2 { rate } else { voltage() },
            ),
            (child_out, outputs[i], SignalDirection::Output, voltage()),
        ] {
            let relay = Id::<kinds::Port>::new();
            nodes.push(PortDef::signal(relay, direction, ty(dimension)).into());
            connect(&mut t, relay.erase(), clock.erase(), EdgeKind::ClockedBy);
            for (driver, sink) in [(source, relay), (relay, sink)] {
                let c = Id::<kinds::Connection>::new();
                nodes.push(ConnectionDef::new(c, ConnectionSemantics::Signal { driver }).into());
                connect(&mut t, c.erase(), driver.erase(), EdgeKind::Connects);
                connect(&mut t, c.erase(), sink.erase(), EdgeKind::Connects);
            }
        }
        let mut d = ExprDagBuilder::new();
        let x = d.symbol(SymbolRef::Field(fields[i])).unwrap();
        let v = d
            .constant(DynQuantity::new([5.0, -3.0, 1.0][i], voltage()))
            .unwrap();

        relation(
            &mut t,
            &mut nodes,
            d.finish([x, v]).unwrap(),
            true,
            activation,
        );
        let mut d = ExprDagBuilder::new();
        let pre = d.symbol(SymbolRef::Pre(fields[i])).unwrap();
        let next = d.symbol(SymbolRef::Next(fields[i])).unwrap();
        let input = d.symbol(SymbolRef::Port(child_in)).unwrap();
        let output = d.symbol(SymbolRef::Port(child_out)).unwrap();
        let rhs = if i == 2 {
            let h = d
                .constant(DynQuantity::new(
                    period as f64 / 1000.0,
                    DimExponents::from_integers([0, 0, 1, 0, 0, 0, 0]).unwrap(),
                ))
                .unwrap();
            let step = d.mul(h, input).unwrap();
            d.add(pre, step).unwrap()
        } else {
            input
        };
        let update = if failure == Failure::Tick && i == 0 {
            let square = d.mul(next, next).unwrap();
            let scale = d.constant(DynQuantity::new(1.0, voltage())).unwrap();
            let target = d.mul(input, scale).unwrap();
            [square, target]
        } else {
            [next, rhs]
        };
        let expose = [output, if i == 2 { next } else { pre }];
        let roots = if reverse {
            vec![expose, update]
        } else {
            vec![update, expose]
        };
        relation(
            &mut t,
            &mut nodes,
            d.finish(roots.into_iter().flatten()).unwrap(),
            false,
            activation,
        );
    }
    if failure == Failure::Consistency {
        let algebraic = Id::<kinds::Field>::new();
        let continuous = Id::<kinds::Activation>::new();
        nodes.push(FieldDef::new(algebraic, ty(voltage()), FieldRole::Variable).into());
        nodes.push(ActivationDef::continuous(continuous).into());
        let mut dag = ExprDagBuilder::new();
        let value = dag.symbol(SymbolRef::Field(algebraic)).unwrap();
        let square = dag.mul(value, value).unwrap();
        let memory = dag.symbol(SymbolRef::Field(fields[0])).unwrap();
        let held = dag.hold(memory).unwrap();
        let scale = dag.constant(DynQuantity::new(1., voltage())).unwrap();
        let target = dag.mul(held, scale).unwrap();

        relation(
            &mut t,
            &mut nodes,
            dag.finish([square, target]).unwrap(),
            false,
            continuous,
        );
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let edges = t;
    let mut t = Transaction::new("three independent sampled memories");
    for node in nodes {
        t.push(Op::DefineKernelNode { node });
    }
    for edge in edges.ops() {
        t.push(edge.clone());
    }
    t.push(Op::DefineOntologyView {
        view: ModelView::new(
            model,
            members,
            inputs.iter().chain(&outputs).map(|id| id.erase()),
        )
        .unwrap()
        .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(t).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    Fixture {
        program,
        clock: clock.erase(),
        inputs: inputs.map(Id::erase),
        outputs: outputs.map(Id::erase),
        fields: fields.map(Id::erase),
    }
}
fn input_tables(f: &Fixture) -> Vec<(RawId, RawId, Vec<ValueLiteral>)> {
    f.inputs
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let KernelNode::Port(port) = f.program.node(*id).unwrap() else {
                panic!()
            };
            let (_, kind) = port.signal_contract().unwrap();
            (
                *id,
                f.clock,
                if i == 0 { [2., -1., 4.] } else { [1., 2., -1.] }
                    .into_iter()
                    .map(|v| ValueLiteral::from_real(kind.clone(), v).unwrap())
                    .collect(),
            )
        })
        .collect()
}
#[test]
fn two_delays_integrator_exact_tables_phase_and_checkpoint_restart() {
    for (period, phase) in [(10, 0), (20, 0), (10, 30)] {
        for reverse in [false, true] {
            let f = fixture(period, phase, reverse, Failure::None);
            let end = (phase + 2 * period) as f64 / 1000.;
            let config = ReferenceConfig::new(end, 0.01).unwrap();
            let mut s = Interpreter::new()
                .sampled_session(&f.program, config, input_tables(&f))
                .unwrap();
            assert_eq!(
                s.field(f.fields[0])
                    .unwrap()
                    .real_scalar_value()
                    .unwrap()
                    .value(),
                5.
            );
            assert!(s.output(f.outputs[0], 0).is_none());
            assert_eq!(s.advance_ticks(2).unwrap(), 2);
            let checkpoint = s.checkpoint();
            let mut resumed = Interpreter::new()
                .resume_sampled(&f.program, &checkpoint)
                .unwrap();
            assert_eq!(
                resumed.next_tick(),
                Some(RationalTime::new(phase + 2 * period, 1000).unwrap())
            );
            assert_eq!(resumed.advance_ticks(1).unwrap(), 1);
            assert_eq!(s.advance_ticks(1).unwrap(), 1);
            for (i, expected) in [
                [5., 2., -1.],
                [-3., 2., -1.],
                [
                    1. + period as f64 / 1000.,
                    1. + 3. * period as f64 / 1000.,
                    1. + 2. * period as f64 / 1000.,
                ],
            ]
            .iter()
            .enumerate()
            {
                for (tick, value) in expected.iter().enumerate() {
                    let (time, actual) = resumed.output(f.outputs[i], tick as u64).unwrap();
                    assert_eq!(
                        time,
                        RationalTime::new(phase + period * tick as u64, 1000).unwrap()
                    );
                    assert!((actual.real_scalar_value().unwrap().value() - value).abs() < 1e-9);
                    assert_eq!(
                        resumed.output(f.outputs[i], tick as u64),
                        s.output(f.outputs[i], tick as u64)
                    );
                }
            }
            assert!(resumed.next_tick().is_none());
            assert_eq!(resumed.advance_ticks(1).unwrap(), 0);
            let other = fixture(period, phase, false, Failure::None);
            assert!(
                Interpreter::new()
                    .resume_sampled(&other.program, &checkpoint)
                    .is_err()
            );
        }
    }
}
#[test]
fn sampled_inputs_reject_missing_duplicate_foreign_clock_and_partial_coverage() {
    let f = fixture(10, 0, false, Failure::None);
    let c = ReferenceConfig::new(0.02, 0.01).unwrap();
    let tables = input_tables(&f);
    assert!(
        Interpreter::new()
            .sampled_session(&f.program, c, tables[..1].to_vec())
            .is_err()
    );
    let mut duplicate = tables.clone();
    duplicate.push(tables[0].clone());
    assert!(
        Interpreter::new()
            .sampled_session(&f.program, c, duplicate)
            .is_err()
    );
    let mut wrong = tables.clone();
    wrong[0].1 = Id::<kinds::ClockDomain>::new().erase();
    assert!(
        Interpreter::new()
            .sampled_session(&f.program, c, wrong)
            .is_err()
    );
    let mut short = tables;
    short[0].2.pop();
    assert!(
        Interpreter::new()
            .sampled_session(&f.program, c, short)
            .is_err()
    );
}
#[test]
fn failed_tick_retains_calendar_memories_inputs_and_output_presence() {
    for failure in [Failure::Tick, Failure::Consistency] {
        let f = fixture(10, 0, false, failure);
        let mut s = Interpreter::new()
            .sampled_session(
                &f.program,
                ReferenceConfig::new(0.02, 0.01)
                    .unwrap()
                    .with_initial_guess(1.)
                    .unwrap(),
                input_tables(&f),
            )
            .unwrap();
        s.advance_ticks(1).unwrap();
        let before = s.checkpoint();
        let time = s.next_tick();
        let memories = f.fields.map(|id| s.field(id));
        assert!(s.advance_ticks(1).is_err());
        assert_eq!(s.next_tick(), time);
        assert_eq!(f.fields.map(|id| s.field(id)), memories);
        assert!(s.output(f.outputs[0], 1).is_none());
        let mut resumed = Interpreter::new()
            .resume_sampled(&f.program, &before)
            .unwrap();
        assert!(resumed.advance_ticks(1).is_err());
        assert_eq!(resumed.next_tick(), time);
    }
}

struct Forwarding {
    program: KernelProgram,
    inputs: [RawId; 2],
    outputs: [RawId; 2],
    clocks: [RawId; 2],
}

fn forwarding(
    second_period: u64,
    mismatch: bool,
    duplicate: bool,
) -> Result<Forwarding, Vec<eqiora_core::Diagnostic>> {
    forwarding_with_periods(
        [
            RationalTime::new(10, 1000).unwrap(),
            RationalTime::new(second_period, 1000).unwrap(),
        ],
        mismatch,
        duplicate,
    )
}

fn forwarding_with_periods(
    periods: [RationalTime; 2],
    mismatch: bool,
    duplicate: bool,
) -> Result<Forwarding, Vec<eqiora_core::Diagnostic>> {
    let clocks = [Id::<kinds::ClockDomain>::new(), Id::new()];
    let inputs = [Id::<kinds::Port>::new(), Id::new()];
    let outputs = [Id::<kinds::Port>::new(), Id::new()];
    let model = OntologyId::<Model>::new();
    let mut t = Transaction::new("independent exact boundary clocks");
    let mut members = Vec::new();
    let mut edges = Vec::new();
    for i in 0..2 {
        let connection = Id::<kinds::Connection>::new();
        let nodes = [
            KernelNode::from(
                ClockDomainDef::periodic(clocks[i], periods[i], RationalTime::ZERO).unwrap(),
            ),
            PortDef::signal(inputs[i], SignalDirection::Input, value_type()).into(),
            PortDef::signal(outputs[i], SignalDirection::Output, value_type()).into(),
            ConnectionDef::new(
                connection,
                ConnectionSemantics::Signal { driver: inputs[i] },
            )
            .into(),
        ];
        for node in nodes {
            members.push(node.id());
            t.push(Op::DefineKernelNode { node });
        }
        edges.extend([
            (inputs[i].erase(), clocks[i].erase(), EdgeKind::ClockedBy),
            (
                outputs[i].erase(),
                clocks[if mismatch { 1 - i } else { i }].erase(),
                EdgeKind::ClockedBy,
            ),
            (connection.erase(), inputs[i].erase(), EdgeKind::Connects),
            (connection.erase(), outputs[i].erase(), EdgeKind::Connects),
        ]);
    }
    if duplicate {
        let connection = Id::<kinds::Connection>::new();
        members.push(connection.erase());
        t.push(Op::DefineKernelNode {
            node: ConnectionDef::new(
                connection,
                ConnectionSemantics::Signal { driver: inputs[0] },
            )
            .into(),
        });
        edges.extend([
            (connection.erase(), inputs[0].erase(), EdgeKind::Connects),
            (connection.erase(), outputs[0].erase(), EdgeKind::Connects),
        ]);
    }
    let mut dag = ExprDagBuilder::new();
    let fixed = Id::<kinds::Field>::new();
    members.push(fixed.erase());
    t.push(Op::DefineKernelNode {
        node: FieldDef::new(fixed, value_type(), FieldRole::Variable).into(),
    });
    let zero = dag.symbol(SymbolRef::Field(fixed)).unwrap();
    let relation = Id::<kinds::Relation>::new();
    members.push(relation.erase());
    edges.push((relation.erase(), fixed.erase(), EdgeKind::DependsOn));
    t.push(Op::DefineKernelNode {
        node: RelationDef::new(
            relation,
            {
                let equation_zero = dag
                    .constant(eqiora_core::DynQuantity::new(0.0, value_type().dimension()))
                    .unwrap();
                dag.finish([zero, equation_zero])
            }
            .unwrap(),
        )
        .unwrap()
        .into(),
    });
    let activation = Id::<kinds::Activation>::new();
    members.push(activation.erase());
    t.push(Op::DefineKernelNode {
        node: ActivationDef::continuous(activation).into(),
    });
    edges.push((activation.erase(), relation.erase(), EdgeKind::Activates));
    for (from, to, edge) in edges {
        connect(&mut t, from, to, edge);
    }
    t.push(Op::DefineOntologyView {
        view: ModelView::new(
            model,
            members,
            inputs.iter().chain(&outputs).map(|id| id.erase()),
        )
        .unwrap()
        .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(t)?;
    Ok(Forwarding {
        program: KernelProgram::from_snapshot(&store.snapshot(), model)?,
        inputs: inputs.map(Id::erase),
        outputs: outputs.map(Id::erase),
        clocks: clocks.map(Id::erase),
    })
}

fn value_type() -> ValueType {
    ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
}

#[test]
fn independent_clocks_forward_only_present_samples_and_keep_nominal_identity() {
    assert!(forwarding(10, true, false).is_err());
    assert!(forwarding(10, false, true).is_err());
    for second_period in [10, 20] {
        let Forwarding {
            program,
            inputs,
            outputs,
            clocks,
        } = forwarding(second_period, false, false).unwrap();
        let literal = |x| ValueLiteral::from_real(value_type(), x).unwrap();
        let second = if second_period == 10 {
            vec![7., 8., 9.]
        } else {
            vec![7., 9.]
        };
        let mut session = Interpreter::new()
            .sampled_session(
                &program,
                ReferenceConfig::new(0.02, 0.01).unwrap(),
                [
                    (
                        inputs[0],
                        clocks[0],
                        [1., 2., 3.].into_iter().map(literal).collect(),
                    ),
                    (
                        inputs[1],
                        clocks[1],
                        second.into_iter().map(literal).collect(),
                    ),
                ],
            )
            .unwrap();
        assert_eq!(session.advance_ticks(2).unwrap(), 2);
        assert_eq!(
            session
                .output(outputs[0], 1)
                .unwrap()
                .1
                .real_scalar_value()
                .unwrap()
                .value(),
            2.
        );
        assert_eq!(session.output(outputs[1], 1).is_some(), second_period == 10);
        assert_eq!(session.advance_ticks(1).unwrap(), 1);
        let final_index = if second_period == 10 { 2 } else { 1 };
        assert_eq!(
            session.output(outputs[1], final_index).unwrap().0,
            RationalTime::new(1, 50).unwrap()
        );
        assert_eq!(
            session
                .output(outputs[1], final_index)
                .unwrap()
                .1
                .real_scalar_value()
                .unwrap()
                .value(),
            9.
        );
    }
}

#[test]
fn sampled_horizon_compares_exact_ticks_to_the_exact_binary64_bound() {
    let denominator = 1_u64 << 53;
    for (numerator, count) in [(denominator + 1, 1), (denominator, 2), (denominator - 1, 2)] {
        let period = RationalTime::new(numerator, denominator).unwrap();
        let Forwarding {
            program,
            inputs,
            outputs,
            clocks,
        } = forwarding_with_periods([period; 2], false, false).unwrap();
        let table = vec![ValueLiteral::from_real(value_type(), 7.).unwrap(); count];
        let mut session = Interpreter::new()
            .sampled_session(
                &program,
                ReferenceConfig::new(1., 1.).unwrap(),
                [
                    (inputs[0], clocks[0], table.clone()),
                    (inputs[1], clocks[1], table),
                ],
            )
            .unwrap();
        assert_eq!(session.advance_ticks(3).unwrap(), count);
        assert_eq!(session.next_tick(), None);
        assert_eq!(session.output(outputs[0], 0).unwrap().0, RationalTime::ZERO);
        assert_eq!(session.output(outputs[0], 1).is_some(), count == 2);
        if count == 2 {
            assert_eq!(session.output(outputs[0], 1).unwrap().0, period);
        }
    }
}
