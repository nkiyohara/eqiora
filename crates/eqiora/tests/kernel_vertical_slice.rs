//! Public-facade vertical slice: construct one canonical relation network
//! without depending on internal `eqiora-*` crates.

use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora::kernel::{
    ActivationDef, ClockDomainDef, DomainDef, ExprDagBuilder, FieldDef, KernelNode, PortDef,
    RationalTime, RelationDef, SignalDirection,
};
use eqiora::ontology::{ModelView, OntologyId};
use eqiora::sem::KernelProgram;
use eqiora::{DimExponents, DynQuantity, Id, kinds};

#[test]
fn public_api_builds_a_clocked_relation_network() {
    let domain = Id::<kinds::Domain>::new();
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let port = Id::<kinds::Port>::new();
    let activation = Id::<kinds::Activation>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let model = ModelView::new(
        OntologyId::new(),
        [
            domain.erase(),
            field.erase(),
            relation.erase(),
            port.erase(),
            activation.erase(),
            clock.erase(),
        ],
        [port.erase()],
    )
    .expect("model view is structurally valid");
    let typed_model_id = model.id();
    let model_id = typed_model_id.erase();

    let mut expressions = ExprDagBuilder::new();
    let field_value = expressions
        .symbol(eqiora::kernel::SymbolRef::Field(field))
        .expect("symbol");
    let zero = expressions
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .expect("constant");
    let residual = expressions.sub(field_value, zero).expect("subtraction");
    let relation_definition = RelationDef::new(
        relation,
        expressions.finish([residual]).expect("residual DAG"),
    );
    let period = RationalTime::new(1, 100).expect("10 ms");

    let mut transaction = Transaction::new("define a clocked relation");
    for node in [
        KernelNode::from(DomainDef::new(domain)),
        KernelNode::from(FieldDef::new(field, DimExponents::DIMENSIONLESS)),
        KernelNode::from(relation_definition),
        KernelNode::from(PortDef::signal(
            port,
            SignalDirection::Output,
            DimExponents::DIMENSIONLESS,
        )),
        KernelNode::from(ActivationDef::periodic(activation)),
        KernelNode::from(
            ClockDomainDef::periodic(clock, period, RationalTime::ZERO)
                .expect("positive exact period"),
        ),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
        .push(Op::Connect {
            from: field.erase(),
            to: domain.erase(),
            edge: EdgeKind::DefinedOn,
        })
        .push(Op::Connect {
            from: relation.erase(),
            to: field.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: clock.erase(),
            edge: EdgeKind::ClockedBy,
        })
        .push(Op::DefineOntologyView { view: model.into() });

    let mut store = InMemoryGraphStore::new();
    let committed = store.commit(transaction).expect("kernel graph is valid");
    let snapshot = store.snapshot();

    assert_eq!(snapshot.edges().len(), 4);
    assert_eq!(snapshot.nodes().len(), 7); // six kernel nodes + provenance
    assert_eq!(snapshot.ontology_views().len(), 1);
    assert!(snapshot.ontology_view(&model_id).is_some());
    assert_eq!(snapshot.commits()[0].transaction(), committed.transaction);
    assert!(KernelProgram::from_snapshot(&snapshot, typed_model_id).is_ok());
}
