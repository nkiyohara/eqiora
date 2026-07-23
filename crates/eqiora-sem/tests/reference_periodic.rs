use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ClockDomainDef, ExprDagBuilder, FieldDef, KernelNode, RationalTime, RelationDef,
    SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};

#[test]
fn coincident_periodic_activations_commit_next_fields_simultaneously() {
    let left = Id::<kinds::Field>::new();
    let right = Id::<kinds::Field>::new();
    let left_relation = Id::<kinds::Relation>::new();
    let right_relation = Id::<kinds::Relation>::new();
    let left_activation = Id::<kinds::Activation>::new();
    let right_activation = Id::<kinds::Activation>::new();
    let left_clock = Id::<kinds::ClockDomain>::new();
    let right_clock = Id::<kinds::ClockDomain>::new();
    let model = OntologyId::<Model>::new();

    let mut left_update = ExprDagBuilder::new();
    let next_left = left_update
        .symbol(SymbolRef::Next(left))
        .expect("next left");
    let pre_right = left_update
        .symbol(SymbolRef::Pre(right))
        .expect("pre right");
    let left_residual = left_update.sub(next_left, pre_right).expect("left update");

    let mut right_update = ExprDagBuilder::new();
    let next_right = right_update
        .symbol(SymbolRef::Next(right))
        .expect("next right");
    let pre_left = right_update.symbol(SymbolRef::Pre(left)).expect("pre left");
    let right_residual = right_update
        .sub(next_right, pre_left)
        .expect("right update");

    let period = RationalTime::new(1, 10).expect("100 ms");
    let nodes = [
        KernelNode::from(
            FieldDef::new(left, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                .expect("left initial"),
        ),
        KernelNode::from(
            FieldDef::new(right, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
                .expect("right initial"),
        ),
        KernelNode::from(RelationDef::new(
            left_relation,
            left_update.finish([left_residual]).expect("left DAG"),
        )),
        KernelNode::from(RelationDef::new(
            right_relation,
            right_update.finish([right_residual]).expect("right DAG"),
        )),
        KernelNode::from(ActivationDef::periodic(left_activation)),
        KernelNode::from(ActivationDef::periodic(right_activation)),
        KernelNode::from(
            ClockDomainDef::periodic(left_clock, period, RationalTime::ZERO).expect("left clock"),
        ),
        KernelNode::from(
            ClockDomainDef::periodic(
                right_clock,
                RationalTime::new(2, 20).expect("same exact period"),
                RationalTime::ZERO,
            )
            .expect("right clock"),
        ),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("simultaneous periodic swap");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
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
        (right_activation, right_relation, right_clock),
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
