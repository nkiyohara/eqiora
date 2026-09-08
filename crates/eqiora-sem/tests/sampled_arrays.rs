//! Whole channel values update simultaneously, with exact rollback and restart.
use eqiora_core::{
    DimExponents, Id, OntologyId, RawId, ScalarDomain, ValueLiteral, ValueType, entity::kinds,
};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::*;
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};

fn scalar(domain: ScalarDomain, value: i64) -> ValueLiteral {
    let ty = ValueType::scalar(domain, DimExponents::DIMENSIONLESS);
    if domain == ScalarDomain::Integer {
        ValueLiteral::from_integer(ty, value).unwrap()
    } else {
        ValueLiteral::from_real(ty, value as f64).unwrap()
    }
}

fn channels(domain: ScalarDomain, values: [i64; 2]) -> ValueLiteral {
    let ty = ValueType::scalar(domain, DimExponents::DIMENSIONLESS)
        .array(2)
        .unwrap();
    if domain == ScalarDomain::Integer {
        ValueLiteral::integer(ty, values).unwrap()
    } else {
        ValueLiteral::new(ty, values.map(|value| (value as f64, 0.0))).unwrap()
    }
}

fn fixture(
    domain: ScalarDomain,
    reverse: bool,
    overflow: bool,
) -> (KernelProgram, [RawId; 2], [RawId; 2]) {
    let model = OntologyId::<Model>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let activation = Id::<kinds::Activation>::new();
    let fields = [Id::<kinds::Field>::new(), Id::new()];
    let outputs = [Id::<kinds::Port>::new(), Id::new()];
    let ty = channels(domain, [0, 0]).value_type().clone();
    let mut nodes = vec![
        ClockDomainDef::periodic(clock, RationalTime::new(1, 1).unwrap(), RationalTime::ZERO)
            .unwrap()
            .into(),
        ActivationDef::periodic(activation).into(),
    ];
    let mut edges = Transaction::new("array dependencies");
    for from in [
        activation.erase(),
        fields[0].erase(),
        fields[1].erase(),
        outputs[0].erase(),
        outputs[1].erase(),
    ] {
        edges.push(Op::Connect {
            from,
            to: clock.erase(),
            edge: EdgeKind::ClockedBy,
        });
    }
    for index in 0..2 {
        nodes.push(FieldDef::new(fields[index], ty.clone(), FieldRole::State).into());
        nodes.push(PortDef::signal(outputs[index], SignalDirection::Output, ty.clone()).into());
    }
    for initial in [true, false] {
        let relation = Id::<kinds::Relation>::new();
        let mut dag = ExprDagBuilder::new();
        let roots = if initial {
            let a = dag.symbol(SymbolRef::Pre(fields[0])).unwrap();
            let b = dag.symbol(SymbolRef::Pre(fields[1])).unwrap();
            let av = dag.constant(channels(domain, [1, 2])).unwrap();
            let bv = dag
                .constant(channels(domain, [3, if overflow { i64::MAX } else { 4 }]))
                .unwrap();
            vec![a, av, b, bv]
        } else {
            let a = dag.symbol(SymbolRef::Pre(fields[0])).unwrap();
            let b = dag.symbol(SymbolRef::Pre(fields[1])).unwrap();
            let a0 = dag.index(a, 0).unwrap();
            let a1 = dag.index(a, 1).unwrap();
            let b0 = dag.index(b, 0).unwrap();
            let b1 = dag.index(b, 1).unwrap();
            let one = dag.constant(scalar(domain, 1)).unwrap();
            let inc_a = dag.add(a0, one).unwrap();
            let inc_b = dag.add(b1, one).unwrap();
            let av = dag.array([b0, inc_a]).unwrap();
            let bv = dag.array([a1, inc_b]).unwrap();
            let next_a = dag.symbol(SymbolRef::Next(fields[0])).unwrap();
            let next_b = dag.symbol(SymbolRef::Next(fields[1])).unwrap();
            let out_a = dag.symbol(SymbolRef::Port(outputs[0])).unwrap();
            let out_b = dag.symbol(SymbolRef::Port(outputs[1])).unwrap();
            let mut equations = vec![(next_a, av), (next_b, bv), (out_a, next_a), (out_b, next_b)];
            if reverse {
                equations.reverse();
            }
            equations
                .into_iter()
                .flat_map(|(lhs, rhs)| [lhs, rhs])
                .collect()
        };
        let expression = dag.finish(roots).unwrap();
        nodes.push(
            if initial {
                RelationDef::initial(relation, expression).unwrap()
            } else {
                RelationDef::new(relation, expression).unwrap()
            }
            .into(),
        );
        for to in fields.map(Id::erase).into_iter().chain(
            (!initial)
                .then_some(outputs.map(Id::erase))
                .into_iter()
                .flatten(),
        ) {
            edges.push(Op::Connect {
                from: relation.erase(),
                to,
                edge: EdgeKind::DependsOn,
            });
        }
        if !initial {
            edges.push(Op::Connect {
                from: activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            });
        }
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("coupled sampled arrays");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for op in edges.ops() {
        transaction.push(op.clone());
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, outputs.map(Id::erase))
            .unwrap()
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    (
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        fields.map(Id::erase),
        outputs.map(Id::erase),
    )
}

#[test]
fn coupled_channels_use_pre_values_in_both_equation_orders_and_resume_exactly() {
    for domain in [ScalarDomain::Real, ScalarDomain::Integer] {
        for reverse in [false, true] {
            let (program, fields, outputs) = fixture(domain, reverse, false);
            let interpreter = Interpreter::new();
            let config = ReferenceConfig::new(1.0, 1.0).unwrap();
            let mut session = interpreter.sampled_session(&program, config, []).unwrap();
            assert_eq!(session.field(fields[0]), Some(channels(domain, [1, 2])));
            assert!(session.output(outputs[0], 0).is_none());
            assert_eq!(session.advance_ticks(1).unwrap(), 1);
            let mut resumed = interpreter
                .resume_sampled(&program, &session.checkpoint())
                .unwrap();
            assert_eq!(resumed.advance_ticks(1).unwrap(), 1);
            // a'=[b0,a0+1], b'=[a1,b1+1], using the complete previous row.
            for (tick, expected) in [[[3, 2], [2, 5]], [[2, 4], [2, 6]]].into_iter().enumerate() {
                for (output, values) in outputs.into_iter().zip(expected) {
                    let (time, actual) = resumed.output(output, tick as u64).unwrap();
                    assert_eq!(time, RationalTime::new(tick as u64, 1).unwrap());
                    assert_eq!(actual, &channels(domain, values));
                }
            }
            assert_eq!(resumed.field(fields[0]), Some(channels(domain, [2, 4])));
            assert_eq!(resumed.field(fields[1]), Some(channels(domain, [2, 6])));
            assert!(interpreter.run(&program, config).is_err());
        }
    }
}

#[test]
fn late_channel_overflow_rolls_back_earlier_staging_calendar_and_outputs() {
    let (program, fields, outputs) = fixture(ScalarDomain::Integer, false, true);
    let interpreter = Interpreter::new();
    let mut session = interpreter
        .sampled_session(&program, ReferenceConfig::new(1., 1.).unwrap(), [])
        .unwrap();
    let checkpoint = session.checkpoint();
    assert!(session.advance_ticks(1).is_err());
    for field in fields {
        assert_eq!(session.field(field), checkpoint.field(field));
    }
    assert_eq!(session.next_tick(), Some(RationalTime::ZERO));
    for output in outputs {
        assert!(session.output(output, 0).is_none());
    }
}
