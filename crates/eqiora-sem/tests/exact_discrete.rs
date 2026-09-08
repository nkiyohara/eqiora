use eqiora_core::{Id, OntologyId, RawId, ValueLiteral, ValueType, entity::kinds};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::*;
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};

fn fixture(
    initial: ValueLiteral,
    change: ValueLiteral,
    nominal: Option<KernelNode>,
) -> (KernelProgram, RawId, RawId, RawId, RawId) {
    let model = OntologyId::<Model>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let activation = Id::<kinds::Activation>::new();
    let field = Id::<kinds::Field>::new();
    let output = Id::<kinds::Port>::new();
    let input = Id::<kinds::Port>::new();
    let mut nodes = vec![
        ClockDomainDef::periodic(clock, RationalTime::new(1, 1).unwrap(), RationalTime::ZERO)
            .unwrap()
            .into(),
        ActivationDef::periodic(activation).into(),
        FieldDef::new(field, initial.value_type().clone(), FieldRole::State).into(),
        PortDef::signal(
            output,
            SignalDirection::Output,
            initial.value_type().clone(),
        )
        .into(),
    ];
    nodes.push(PortDef::signal(input, SignalDirection::Input, change.value_type().clone()).into());
    if let Some(node) = nominal {
        nodes.push(node);
    }
    let mut transaction = Transaction::new("exact discrete state");
    for from in [
        activation.erase(),
        field.erase(),
        output.erase(),
        input.erase(),
    ] {
        transaction.push(Op::Connect {
            from,
            to: clock.erase(),
            edge: EdgeKind::ClockedBy,
        });
    }
    for is_initial in [true, false] {
        let relation = Id::<kinds::Relation>::new();
        let mut dag = ExprDagBuilder::new();
        let mut dependencies = vec![field.erase()];
        let roots = if is_initial {
            let target = dag.symbol(SymbolRef::Pre(field)).unwrap();
            let value = dag.constant(initial.clone()).unwrap();
            vec![dag.sub(target, value).unwrap()]
        } else {
            let next = dag.symbol(SymbolRef::Next(field)).unwrap();
            let pre = dag.symbol(SymbolRef::Pre(field)).unwrap();
            let delta = dag.symbol(SymbolRef::Port(input)).unwrap();
            dependencies.push(input.erase());
            let rhs = dag.add(pre, delta).unwrap();
            let out = dag.symbol(SymbolRef::Port(output)).unwrap();
            dependencies.push(output.erase());
            vec![dag.sub(next, rhs).unwrap(), dag.sub(out, next).unwrap()]
        };
        let dag = dag.finish(roots).unwrap();
        nodes.push(
            if is_initial {
                RelationDef::initial(relation, dag)
            } else {
                RelationDef::new(relation, dag)
            }
            .into(),
        );
        for to in dependencies {
            transaction.push(Op::Connect {
                from: relation.erase(),
                to,
                edge: EdgeKind::DependsOn,
            });
        }
        if !is_initial {
            transaction.push(Op::Connect {
                from: activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            });
        }
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let edges = transaction;
    let mut transaction = Transaction::new("exact discrete state");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for op in edges.ops() {
        transaction.push(op.clone());
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, [output.erase(), input.erase()])
            .unwrap()
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    (
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        field.erase(),
        output.erase(),
        input.erase(),
        clock.erase(),
    )
}

#[test]
fn exact_species_transfer_is_atomic_and_resumes_after_second_tick() {
    let space = FiniteSpaceDef::new(Id::new(), ["A".into(), "B".into()]).unwrap();
    let initial = ValueLiteral::integer(space.counts(), [2, 9_007_199_254_740_993]).unwrap();
    let change = ValueLiteral::integer(space.coordinates(), [-1, 1]).unwrap();
    let (program, field, output, input, clock) =
        fixture(initial.clone(), change.clone(), Some(space.into()));
    let interpreter = Interpreter::new();
    assert!(
        interpreter
            .run(&program, ReferenceConfig::new(2., 1.).unwrap())
            .is_err()
    );
    assert_eq!(
        interpreter
            .initialize(&program, ReferenceConfig::new(2., 1.).unwrap())
            .unwrap()
            .fields()[&field],
        initial
    );
    let mut session = interpreter
        .sampled_session(
            &program,
            ReferenceConfig::new(2., 1.).unwrap(),
            [(input, clock, vec![change; 3])],
        )
        .unwrap();
    for (tick, expected) in [[1, 9_007_199_254_740_994], [0, 9_007_199_254_740_995]]
        .into_iter()
        .enumerate()
    {
        assert_eq!(session.advance_ticks(1).unwrap(), 1);
        let value = session.field(field).unwrap();
        assert_eq!(
            [
                value.integer_component(0).unwrap(),
                value.integer_component(1).unwrap()
            ],
            expected
        );
        assert_eq!(session.output(output, tick as u64).unwrap().1, &value);
    }
    let checkpoint = session.checkpoint();
    let mut resumed = interpreter.resume_sampled(&program, &checkpoint).unwrap();
    assert!(resumed.advance_ticks(1).is_err());
    assert_eq!(resumed.field(field), checkpoint.field(field));
    assert_eq!(resumed.next_tick(), checkpoint.next_tick());
    assert_eq!(resumed.output(output, 1), checkpoint.output(output, 1));
    assert!(resumed.output(output, 2).is_none());
}

#[test]
fn integer_overflow_keeps_accepted_state_and_output_absence() {
    let ty = ValueType::scalar(
        eqiora_core::ScalarDomain::Integer,
        eqiora_core::DimExponents::DIMENSIONLESS,
    );
    let initial = ValueLiteral::from_integer(ty.clone(), i64::MAX).unwrap();
    let (program, field, output, input, clock) = fixture(
        initial.clone(),
        ValueLiteral::from_integer(ty.clone(), 1).unwrap(),
        None,
    );
    let mut session = Interpreter::new()
        .sampled_session(
            &program,
            ReferenceConfig::new(1., 1.).unwrap(),
            [(
                input,
                clock,
                vec![ValueLiteral::from_integer(ty, 1).unwrap(); 2],
            )],
        )
        .unwrap();
    assert!(session.advance_ticks(1).is_err());
    assert_eq!(session.field(field), Some(initial));
    assert_eq!(session.next_tick(), Some(RationalTime::ZERO));
    assert!(session.output(output, 0).is_none());
}
