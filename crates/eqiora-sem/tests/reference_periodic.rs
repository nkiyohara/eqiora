#[path = "support/initial.rs"]
mod initial_support;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ClockDomainDef, ExprDagBuilder, FieldDef, KernelNode, RationalTime, RelationDef,
    SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};
use initial_support::{define_all, initial};

#[test]
fn coincident_periodic_activations_commit_next_fields_simultaneously() {
    let left = Id::<kinds::Field>::new();
    let right = Id::<kinds::Field>::new();
    let left_relation = Id::<kinds::Relation>::new();
    let right_relation = Id::<kinds::Relation>::new();
    let left_activation = Id::<kinds::Activation>::new();
    let right_activation = Id::<kinds::Activation>::new();
    let left_clock = Id::<kinds::ClockDomain>::new();
    let model = OntologyId::<Model>::new();

    let mut left_update = ExprDagBuilder::new();
    let next_left = left_update
        .symbol(SymbolRef::Next(left))
        .expect("next left");
    let pre_right = left_update
        .symbol(SymbolRef::Pre(right))
        .expect("pre right");

    let mut right_update = ExprDagBuilder::new();
    let next_right = right_update
        .symbol(SymbolRef::Next(right))
        .expect("next right");
    let pre_left = right_update.symbol(SymbolRef::Pre(left)).expect("pre left");

    let period = RationalTime::new(1, 10).expect("100 ms");
    let nodes = [
        KernelNode::from(FieldDef::new(
            left,
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Real,
                DimExponents::DIMENSIONLESS,
            ),
            eqiora_schema::kernel::FieldRole::State,
        )),
        initial(left, DynQuantity::new(1.0, DimExponents::DIMENSIONLESS)),
        KernelNode::from(FieldDef::new(
            right,
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Real,
                DimExponents::DIMENSIONLESS,
            ),
            eqiora_schema::kernel::FieldRole::State,
        )),
        initial(right, DynQuantity::new(2.0, DimExponents::DIMENSIONLESS)),
        KernelNode::from(
            RelationDef::new(
                left_relation,
                left_update
                    .finish([next_left, pre_right])
                    .expect("left DAG"),
            )
            .unwrap(),
        ),
        KernelNode::from(
            RelationDef::new(
                right_relation,
                right_update
                    .finish([next_right, pre_left])
                    .expect("right DAG"),
            )
            .unwrap(),
        ),
        KernelNode::from(ActivationDef::periodic(left_activation)),
        KernelNode::from(ActivationDef::periodic(right_activation)),
        KernelNode::from(
            ClockDomainDef::periodic(left_clock, period, RationalTime::ZERO).expect("left clock"),
        ),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("simultaneous periodic swap");
    define_all(&mut transaction, nodes);
    for (relation, dependencies) in [
        (left_relation.erase(), [left.erase(), right.erase()]),
        (right_relation.erase(), [right.erase(), left.erase()]),
    ] {
        for dependency in dependencies {
            transaction.push(Op::Connect {
                from: relation,
                to: dependency,
                edge: EdgeKind::DependsOn,
            });
        }
    }
    for (activation, relation, clock) in [
        (left_activation, left_relation, left_clock),
        (right_activation, right_relation, left_clock),
    ] {
        transaction
            .push(Op::Connect {
                from: activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            })
            .push(Op::Connect {
                from: activation.erase(),
                to: clock.erase(),
                edge: EdgeKind::ClockedBy,
            });
    }
    for field in [left, right] {
        transaction.push(Op::Connect {
            from: field.erase(),
            to: left_clock.erase(),
            edge: EdgeKind::ClockedBy,
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, [])
            .expect("ModelView")
            .into(),
    });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("valid graph");
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).expect("valid program");
    let trajectory = Interpreter::new()
        .run(&program, ReferenceConfig::new(0.0, 0.01).expect("config"))
        .expect("simultaneous tick");

    assert_eq!(
        trajectory.last_value(left.erase()).expect("left").value(),
        2.0
    );
    assert_eq!(
        trajectory.last_value(right.erase()).expect("right").value(),
        1.0
    );
}

#[test]
fn equal_periods_do_not_substitute_for_exact_state_clock_ownership() {
    let field = Id::<kinds::Field>::new();
    let state_clock = Id::<kinds::ClockDomain>::new();
    let relation_clock = Id::<kinds::ClockDomain>::new();
    let activation = Id::<kinds::Activation>::new();
    let relation = Id::<kinds::Relation>::new();
    let model = OntologyId::<Model>::new();
    let period = RationalTime::new(1, 10).unwrap();
    let mut dag = ExprDagBuilder::new();
    let next = dag.symbol(SymbolRef::Next(field)).unwrap();
    let pre = dag.symbol(SymbolRef::Pre(field)).unwrap();

    let nodes = vec![
        FieldDef::new(
            field,
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Real,
                DimExponents::DIMENSIONLESS,
            ),
            eqiora_schema::kernel::FieldRole::State,
        )
        .into(),
        RelationDef::new(relation, dag.finish([next, pre]).unwrap())
            .unwrap()
            .into(),
        ActivationDef::periodic(activation).into(),
        ClockDomainDef::periodic(state_clock, period, RationalTime::ZERO)
            .unwrap()
            .into(),
        ClockDomainDef::periodic(relation_clock, period, RationalTime::ZERO)
            .unwrap()
            .into(),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("equal period wrong exact clock");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (from, to, edge) in [
        (field.erase(), state_clock.erase(), EdgeKind::ClockedBy),
        (
            activation.erase(),
            relation_clock.erase(),
            EdgeKind::ClockedBy,
        ),
        (activation.erase(), relation.erase(), EdgeKind::Activates),
        (relation.erase(), field.erase(), EdgeKind::DependsOn),
    ] {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let errors = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("exact Relation ClockDomain"))
    );
}

#[test]
fn reference_run_excludes_an_exact_tick_that_rounds_down_to_the_horizon() {
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let model = OntologyId::<Model>::new();
    let mut dag = ExprDagBuilder::new();
    let pre = dag.symbol(SymbolRef::Pre(field)).unwrap();
    let next = dag.symbol(SymbolRef::Next(field)).unwrap();
    let one = dag
        .constant(DynQuantity::new(1., DimExponents::DIMENSIONLESS))
        .unwrap();
    let increment = dag.add(pre, one).unwrap();

    let nodes = [
        KernelNode::from(FieldDef::new(
            field,
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Real,
                DimExponents::DIMENSIONLESS,
            ),
            eqiora_schema::kernel::FieldRole::State,
        )),
        initial(field, DynQuantity::new(0., DimExponents::DIMENSIONLESS)),
        RelationDef::new(relation, dag.finish([next, increment]).unwrap())
            .unwrap()
            .into(),
        ActivationDef::periodic(activation).into(),
        ClockDomainDef::periodic(
            clock,
            RationalTime::new((1_u64 << 53) + 1, 1_u64 << 53).unwrap(),
            RationalTime::ZERO,
        )
        .unwrap()
        .into(),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("exact closed periodic horizon");
    define_all(&mut transaction, nodes);
    for (from, to, edge) in [
        (relation.erase(), field.erase(), EdgeKind::DependsOn),
        (activation.erase(), relation.erase(), EdgeKind::Activates),
        (activation.erase(), clock.erase(), EdgeKind::ClockedBy),
        (field.erase(), clock.erase(), EdgeKind::ClockedBy),
    ] {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).unwrap().into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let trajectory = Interpreter::new()
        .run(&program, ReferenceConfig::new(1., 1.).unwrap())
        .unwrap();
    // Only tick zero lies in [0, 1]; the next exact tick is 1 + 2^-53.
    assert_eq!(trajectory.last_value(field.erase()).unwrap().value(), 1.);
}
