//! Exact eager selection and explicit sampled-real assignment closure.
use eqiora_core::{
    DimExponents, Id, OntologyId, RawId, ScalarDomain, ValueLiteral, ValueType, entity::kinds,
};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::*;
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};

fn voltage(value: f64) -> ValueLiteral {
    ValueLiteral::from_real(
        ValueType::scalar(
            ScalarDomain::Real,
            DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).unwrap(),
        ),
        value,
    )
    .unwrap()
}
fn fixture(
    implicit: bool,
    continuous: bool,
    failure: bool,
) -> (KernelProgram, RawId, RawId, RawId, RawId) {
    let model = OntologyId::<Model>::new();
    let field = Id::<kinds::Field>::new();
    let doubled = Id::<kinds::Field>::new();
    let input = Id::<kinds::Port>::new();
    let output = Id::<kinds::Port>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let activation = Id::<kinds::Activation>::new();
    let initial = Id::<kinds::Relation>::new();
    let relation = Id::<kinds::Relation>::new();
    let ty = voltage(0.).value_type().clone();
    let mut init = ExprDagBuilder::new();
    let initial_target = init.symbol(SymbolRef::Field(field)).unwrap();
    let zero = init.constant(voltage(0.)).unwrap();
    let mut dag = ExprDagBuilder::new();
    let previous = dag
        .symbol(if continuous {
            SymbolRef::Field(field)
        } else {
            SymbolRef::Pre(field)
        })
        .unwrap();
    let target = dag
        .symbol(if continuous {
            SymbolRef::Field(field)
        } else {
            SymbolRef::Next(field)
        })
        .unwrap();
    let drive = dag.symbol(SymbolRef::Port(input)).unwrap();
    let sum = dag.add(previous, drive).unwrap();
    let cap = dag.constant(voltage(5.)).unwrap();
    let selected = dag.min(sum, cap).unwrap();
    let selected = if failure {
        let badzero = dag
            .constant(
                ValueLiteral::from_real(
                    ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
                    0.,
                )
                .unwrap(),
            )
            .unwrap();
        let invalid = dag.div(cap, badzero).unwrap();
        dag.max(selected, invalid).unwrap()
    } else {
        selected
    };
    let direct = dag.symbol(SymbolRef::Field(doubled)).unwrap();
    let out = dag.symbol(SymbolRef::Port(output)).unwrap();
    let twice = dag.add(target, target).unwrap();
    let roots = if implicit {
        let residual = dag.sub(target, selected).unwrap();
        let zero = dag.constant(voltage(0.)).unwrap();
        vec![residual, zero, direct, twice, out, direct]
    } else {
        vec![target, selected, direct, twice, out, direct]
    };
    let nodes: Vec<KernelNode> = vec![
        FieldDef::new(field, ty.clone(), FieldRole::State).into(),
        FieldDef::new(doubled, ty.clone(), FieldRole::Variable).into(),
        PortDef::signal(input, SignalDirection::Input, ty.clone()).into(),
        PortDef::signal(output, SignalDirection::Output, ty).into(),
        ClockDomainDef::periodic(clock, RationalTime::new(1, 1).unwrap(), RationalTime::ZERO)
            .unwrap()
            .into(),
        (if continuous {
            ActivationDef::continuous(activation)
        } else {
            ActivationDef::periodic(activation)
        })
        .into(),
        RelationDef::initial(initial, init.finish([initial_target, zero]).unwrap())
            .unwrap()
            .into(),
        RelationDef::new(relation, dag.finish(roots).unwrap())
            .unwrap()
            .into(),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut tx = Transaction::new("finite extrema sampled fixture");
    for node in nodes {
        tx.push(Op::DefineKernelNode { node });
    }
    if !continuous {
        for from in [
            field.erase(),
            doubled.erase(),
            input.erase(),
            output.erase(),
            activation.erase(),
        ] {
            tx.push(Op::Connect {
                from,
                to: clock.erase(),
                edge: EdgeKind::ClockedBy,
            });
        }
    }
    tx.push(Op::Connect {
        from: initial.erase(),
        to: field.erase(),
        edge: EdgeKind::DependsOn,
    });
    for to in [
        field.erase(),
        doubled.erase(),
        input.erase(),
        output.erase(),
    ] {
        tx.push(Op::Connect {
            from: relation.erase(),
            to,
            edge: EdgeKind::DependsOn,
        });
    }
    tx.push(Op::Connect {
        from: activation.erase(),
        to: relation.erase(),
        edge: EdgeKind::Activates,
    });
    tx.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, [input.erase(), output.erase()])
            .unwrap()
            .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(tx).unwrap();
    (
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        field.erase(),
        input.erase(),
        output.erase(),
        clock.erase(),
    )
}

#[test]
fn dimensioned_sampled_selection_and_dependent_real_assignment_resume() {
    let (program, field, input, output, clock) = fixture(false, false, false);
    let interpreter = Interpreter::new();
    let mut session = interpreter
        .execution_session(
            &program,
            ReferenceConfig::new(2., 1.).unwrap(),
            [(input, clock, vec![voltage(3.), voltage(4.), voltage(-2.)])],
        )
        .unwrap();
    assert_eq!(session.field(field), Some(voltage(0.)));
    session.advance_ticks(1).unwrap();
    let mut resumed = interpreter
        .resume_execution(&program, &session.checkpoint())
        .unwrap();
    resumed.advance_ticks(2).unwrap();
    for (tick, expected) in [6., 10., 6.].into_iter().enumerate() {
        assert_eq!(
            resumed.output(output, tick as u64).unwrap().1,
            &voltage(expected)
        );
    }
    assert_eq!(resumed.field(field), Some(voltage(3.)));
}

#[test]
fn eager_failure_keeps_state_calendar_and_output_absent() {
    let (program, field, input, output, clock) = fixture(false, false, true);
    let mut session = Interpreter::new()
        .execution_session(
            &program,
            ReferenceConfig::new(0., 1.).unwrap(),
            [(input, clock, vec![voltage(3.)])],
        )
        .unwrap();
    let checkpoint = session.checkpoint();
    assert!(session.advance_ticks(1).is_err());
    assert_eq!(session.field(field), checkpoint.field(field));
    assert_eq!(session.next_tick(), checkpoint.next_tick());
    assert!(session.output(output, 0).is_none());
}

#[test]
fn implicit_selection_uses_common_real_equations() {
    let (program, field, input, output, clock) = fixture(true, false, false);
    let mut session = Interpreter::new()
        .execution_session(
            &program,
            ReferenceConfig::new(0., 1.).unwrap(),
            [(input, clock, vec![voltage(3.)])],
        )
        .unwrap();
    session.advance_ticks(1).unwrap();
    assert_eq!(session.field(field), Some(voltage(3.)));
    assert_eq!(session.output(output, 0).unwrap().1, &voltage(6.));
}

#[test]
fn continuous_selection_has_the_same_numerical_projection_as_select() {
    let (program, _, _, _, _) = fixture(false, true, false);
    for node in program.nodes() {
        if let KernelNode::Relation(relation) = node {
            assert!(program.numerical_residuals(relation.id().erase()).is_ok());
        }
    }
}
