use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, ClockDomainDef, ConnectionDef, ConnectionSemantics, DomainDef,
    EventDirection, ExprDag, ExprDagBuilder, ExprNode, FieldDef, KernelNode, ParameterDef, PortDef,
    RationalTime, RelationDef, SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{KernelProgram, PhysicalUnknown};

#[derive(Clone, Copy)]
struct PhysicalIds {
    domain: Id<kinds::Domain>,
    parameter: Id<kinds::Parameter>,
    connection: Id<kinds::Connection>,
    activation: Id<kinds::Activation>,
    ports: [Id<kinds::Port>; 3],
    relations: [Id<kinds::Relation>; 3],
    model: OntologyId<Model>,
}

fn dimensions() -> (DimExponents, DimExponents) {
    (
        DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).expect("bounded dimension"),
        DimExponents::from_integers([0, 0, 0, 1, 0, 0, 0]).expect("bounded dimension"),
    )
}

fn ids() -> PhysicalIds {
    let mut ports = [Id::new(), Id::new(), Id::new()];
    ports.sort_by_key(|port: &Id<kinds::Port>| port.erase());
    let mut relations = [Id::new(), Id::new(), Id::new()];
    relations.sort_by_key(|relation: &Id<kinds::Relation>| relation.erase());
    PhysicalIds {
        domain: Id::new(),
        parameter: Id::new(),
        connection: Id::new(),
        activation: Id::new(),
        ports,
        relations,
        model: OntologyId::new(),
    }
}

fn physical_transaction(ids: PhysicalIds, reverse_insertion: bool) -> Transaction {
    let (across_dimension, through_dimension) = dimensions();
    let mut nodes = vec![
        KernelNode::from(DomainDef::scalar_physical(
            ids.domain,
            across_dimension,
            through_dimension,
        )),
        KernelNode::from(ParameterDef::new(
            ids.parameter,
            DynQuantity::new(12.0, across_dimension),
        )),
        KernelNode::from(ConnectionDef::new(
            ids.connection,
            ConnectionSemantics::Conserving,
        )),
        KernelNode::from(ActivationDef::continuous(ids.activation)),
    ];
    for (index, (&port, &relation)) in ids.ports.iter().zip(&ids.relations).enumerate() {
        nodes.push(PortDef::scalar_physical(port, ids.domain).into());
        let mut expression = ExprDagBuilder::new();
        let roots = if index == 0 {
            let across = expression.symbol(SymbolRef::Across(port)).unwrap();
            let parameter = expression
                .symbol(SymbolRef::Parameter(ids.parameter))
                .unwrap();
            let constitutive = expression.sub(across, parameter).unwrap();
            let time = expression.symbol(SymbolRef::Time).unwrap();
            vec![constitutive, time]
        } else {
            vec![expression.symbol(SymbolRef::Through(port)).unwrap()]
        };
        nodes.push(RelationDef::new(relation, expression.finish(roots).unwrap()).into());
    }
    if reverse_insertion {
        nodes.reverse();
    }

    let view = ModelView::new(ids.model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("scalar physical network");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (&port, &relation) in ids.ports.iter().zip(&ids.relations) {
        transaction
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::HasPort,
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::DependsOn,
            })
            .push(Op::Connect {
                from: ids.activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            });
    }
    transaction.push(Op::Connect {
        from: ids.relations[0].erase(),
        to: ids.parameter.erase(),
        edge: EdgeKind::DependsOn,
    });
    let ports = if reverse_insertion {
        ids.ports.into_iter().rev().collect::<Vec<_>>()
    } else {
        ids.ports.to_vec()
    };
    for port in ports {
        transaction.push(Op::Connect {
            from: ids.connection.erase(),
            to: port.erase(),
            edge: EdgeKind::Connects,
        });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });
    transaction
}

fn program(ids: PhysicalIds, reverse_insertion: bool) -> KernelProgram {
    let mut store = InMemoryGraphStore::new();
    store
        .commit(physical_transaction(ids, reverse_insertion))
        .unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), ids.model).unwrap()
}

#[test]
fn nary_junction_composition_is_canonical_and_explicit() {
    let ids = ids();
    let first = program(ids, false)
        .compose_scalar_physical_subsystem(ids.connection)
        .unwrap();
    let reordered = program(ids, true)
        .compose_scalar_physical_subsystem(ids.connection)
        .unwrap();

    assert_eq!(first, reordered);
    assert_eq!(first.subsystem().connection(), ids.connection);
    assert_eq!(
        first.unknowns(),
        &[
            PhysicalUnknown::Across(ids.ports[0]),
            PhysicalUnknown::Through(ids.ports[0]),
            PhysicalUnknown::Across(ids.ports[1]),
            PhysicalUnknown::Through(ids.ports[1]),
            PhysicalUnknown::Across(ids.ports[2]),
            PhysicalUnknown::Through(ids.ports[2]),
        ]
    );
    assert_eq!(first.parameters(), &[ids.parameter]);
    assert!(first.uses_time());
    assert_eq!(first.relations().len(), 3);
    assert_eq!(first.relations()[0].dag().roots().len(), 2);

    let junction = first.junctions()[0].dag();
    assert_eq!(junction.roots().len(), 3);
    let symbols = junction
        .nodes()
        .iter()
        .filter_map(|node| match node {
            ExprNode::Symbol(symbol) => Some(*symbol),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        symbols,
        vec![
            SymbolRef::Across(ids.ports[0]),
            SymbolRef::Across(ids.ports[1]),
            SymbolRef::Across(ids.ports[2]),
            SymbolRef::Through(ids.ports[0]),
            SymbolRef::Through(ids.ports[1]),
            SymbolRef::Through(ids.ports[2]),
        ]
    );
}

#[test]
fn composed_system_reference_evaluation_preserves_canonical_root_order() {
    let ids = ids();
    let system = program(ids, false)
        .compose_scalar_physical_subsystem(ids.connection)
        .unwrap();
    let residuals = system
        .evaluate_reference(&[12.0, -3.0, 12.0, 1.0, 12.0, 2.0], &[12.0], Some(0.0))
        .unwrap();

    assert_eq!(residuals, [0.0, 0.0, 1.0, 2.0, 0.0, 0.0, 0.0]);

    let missing_unknown = system
        .evaluate_reference(&[0.0; 5], &[12.0], Some(0.0))
        .unwrap_err();
    assert_eq!(missing_unknown.code(), codes::MISSING_EXECUTION_INPUT);

    let missing_time = system
        .evaluate_reference(&[0.0; 6], &[12.0], None)
        .unwrap_err();
    assert_eq!(missing_time.code(), codes::MISSING_EXECUTION_INPUT);

    let nonfinite_time = system
        .evaluate_reference(&[0.0; 6], &[12.0], Some(f64::NAN))
        .unwrap_err();
    assert_eq!(nonfinite_time.code(), codes::NONFINITE_EVALUATION);
}

fn diagnostics(transaction: Transaction, model: OntologyId<Model>) -> Vec<String> {
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model)
        .unwrap_err()
        .into_iter()
        .map(|diagnostic| diagnostic.message().to_owned())
        .collect()
}

fn constant_guard() -> ExprDag {
    let mut expression = ExprDagBuilder::new();
    let root = expression
        .constant(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    expression.finish([root]).unwrap()
}

fn replace_activation(
    source: &Transaction,
    activation: Id<kinds::Activation>,
    kind: ActivationKind,
) -> Transaction {
    let replacement = KernelNode::from(ActivationDef::new(activation, kind).unwrap());
    let mut transaction = Transaction::new("non-continuous scalar physical network");
    for operation in source.ops() {
        match operation {
            Op::DefineKernelNode { node } if node.id() == activation.erase() => {
                transaction.push(Op::DefineKernelNode {
                    node: replacement.clone(),
                });
            }
            _ => {
                transaction.push(operation.clone());
            }
        }
    }
    transaction
}

fn add_second_connection(source: &Transaction, ids: PhysicalIds) -> Transaction {
    let second = Id::<kinds::Connection>::new();
    let mut transaction = Transaction::new("duplicate physical connection membership");
    for operation in source.ops() {
        match operation {
            Op::DefineOntologyView { view } => {
                let source_view = view.downcast::<Model>().unwrap();
                transaction
                    .push(Op::DefineKernelNode {
                        node: ConnectionDef::new(second, ConnectionSemantics::Conserving).into(),
                    })
                    .push(Op::Connect {
                        from: second.erase(),
                        to: ids.ports[0].erase(),
                        edge: EdgeKind::Connects,
                    });
                let expanded = ModelView::new(
                    ids.model,
                    source_view
                        .members()
                        .iter()
                        .copied()
                        .chain([second.erase()]),
                    source_view.boundary().iter().copied(),
                )
                .unwrap();
                transaction.push(Op::DefineOntologyView {
                    view: expanded.into(),
                });
            }
            _ => {
                transaction.push(operation.clone());
            }
        }
    }
    transaction
}

#[test]
fn physical_symbols_reject_signal_ports() {
    let signal = Id::<kinds::Port>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();
    let mut expression = ExprDagBuilder::new();
    let across = expression.symbol(SymbolRef::Across(signal)).unwrap();
    let nodes = vec![
        KernelNode::from(PortDef::signal(
            signal,
            eqiora_schema::kernel::SignalDirection::Input,
            DimExponents::DIMENSIONLESS,
        )),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([across]).unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("invalid physical symbol");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
        .push(Op::Connect {
            from: relation.erase(),
            to: signal.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView { view: view.into() });

    assert!(
        diagnostics(transaction, model)
            .iter()
            .any(|message| message.contains("Port contract"))
    );
}

#[test]
fn physical_ports_reject_duplicate_membership_and_event_or_guard_activation() {
    let ids = ids();
    let source = physical_transaction(ids, false);
    let duplicate_messages = diagnostics(add_second_connection(&source, ids), ids.model);
    assert!(
        duplicate_messages
            .iter()
            .any(|message| message.contains("exactly one Connection membership"))
    );

    for kind in [
        ActivationKind::Event {
            guard: constant_guard(),
            direction: EventDirection::Any,
        },
        ActivationKind::Guard {
            guard: constant_guard(),
        },
    ] {
        let messages = diagnostics(replace_activation(&source, ids.activation, kind), ids.model);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("continuous Activation"))
        );
    }
}

#[test]
fn physical_relation_admits_state_but_still_requires_continuous_activation_and_closure() {
    let ids = ids();
    let field = Id::<kinds::Field>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let (across_dimension, through_dimension) = dimensions();
    let mut expression = ExprDagBuilder::new();
    let physical = expression.symbol(SymbolRef::Across(ids.ports[0])).unwrap();
    let causal = expression.symbol(SymbolRef::Field(field)).unwrap();
    let root = expression.add(physical, causal).unwrap();
    let relation = RelationDef::new(ids.relations[0], expression.finish([root]).unwrap());
    let periodic = ActivationDef::new(ids.activation, ActivationKind::Periodic).unwrap();
    let nodes = vec![
        DomainDef::scalar_physical(ids.domain, across_dimension, through_dimension).into(),
        PortDef::scalar_physical(ids.ports[0], ids.domain).into(),
        PortDef::scalar_physical(ids.ports[1], ids.domain).into(),
        relation.into(),
        RelationDef::new(ids.relations[1], {
            let mut expression = ExprDagBuilder::new();
            let through = expression.symbol(SymbolRef::Through(ids.ports[1])).unwrap();
            expression.finish([through]).unwrap()
        })
        .into(),
        FieldDef::new(field, across_dimension).into(),
        periodic.into(),
        ClockDomainDef::periodic(clock, RationalTime::new(1, 10).unwrap(), RationalTime::ZERO)
            .unwrap()
            .into(),
        ConnectionDef::new(ids.connection, ConnectionSemantics::Conserving).into(),
    ];
    let view = ModelView::new(
        ids.model,
        nodes.iter().map(KernelNode::id),
        [ids.ports[0].erase()],
    )
    .unwrap();
    let mut transaction = Transaction::new("mixed physical relation");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (&port, &relation) in ids.ports[..2].iter().zip(&ids.relations[..2]) {
        transaction
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::HasPort,
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::DependsOn,
            })
            .push(Op::Connect {
                from: ids.activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            })
            .push(Op::Connect {
                from: ids.connection.erase(),
                to: port.erase(),
                edge: EdgeKind::Connects,
            });
    }
    transaction
        .push(Op::Connect {
            from: ids.relations[0].erase(),
            to: field.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: ids.activation.erase(),
            to: clock.erase(),
            edge: EdgeKind::ClockedBy,
        })
        .push(Op::DefineOntologyView { view: view.into() });

    let messages = diagnostics(transaction, ids.model);
    assert!(
        messages
            .iter()
            .any(|message| message.contains("continuous Activation"))
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.contains("unsupported hybrid or spatial"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("model-root boundary"))
    );
}

#[test]
fn nominal_domain_identity_and_complete_ownership_are_required() {
    let ids = ids();
    let other_domain = Id::<kinds::Domain>::new();
    let (across_dimension, through_dimension) = dimensions();
    let activation = ActivationDef::continuous(ids.activation);
    let connection = ConnectionDef::new(ids.connection, ConnectionSemantics::Conserving);
    let mut nodes = vec![
        DomainDef::scalar_physical(ids.domain, across_dimension, through_dimension).into(),
        DomainDef::scalar_physical(other_domain, across_dimension, through_dimension).into(),
        PortDef::scalar_physical(ids.ports[0], ids.domain).into(),
        PortDef::scalar_physical(ids.ports[1], other_domain).into(),
        activation.into(),
        connection.into(),
    ];
    for (&port, &relation) in ids.ports[..2].iter().zip(&ids.relations[..2]) {
        let mut expression = ExprDagBuilder::new();
        let through = expression.symbol(SymbolRef::Through(port)).unwrap();
        nodes.push(RelationDef::new(relation, expression.finish([through]).unwrap()).into());
    }
    let view = ModelView::new(ids.model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("nominally mismatched junction");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (&port, &relation) in ids.ports[..2].iter().zip(&ids.relations[..2]) {
        transaction
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::HasPort,
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: port.erase(),
                edge: EdgeKind::DependsOn,
            })
            .push(Op::Connect {
                from: ids.activation.erase(),
                to: relation.erase(),
                edge: EdgeKind::Activates,
            })
            .push(Op::Connect {
                from: ids.connection.erase(),
                to: port.erase(),
                edge: EdgeKind::Connects,
            });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let messages = KernelProgram::from_snapshot(&store.snapshot(), ids.model)
        .unwrap_err()
        .into_iter()
        .map(|diagnostic| diagnostic.message().to_owned())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("one exact Domain"))
    );

    let orphan = Id::<kinds::Port>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let connection = Id::<kinds::Connection>::new();
    let model = OntologyId::<Model>::new();
    let mut expression = ExprDagBuilder::new();
    let through = expression.symbol(SymbolRef::Through(orphan)).unwrap();
    let nodes = vec![
        DomainDef::scalar_physical(ids.domain, across_dimension, through_dimension).into(),
        PortDef::scalar_physical(orphan, ids.domain).into(),
        RelationDef::new(relation, expression.finish([through]).unwrap()).into(),
        ActivationDef::continuous(activation).into(),
        ConnectionDef::new(connection, ConnectionSemantics::Conserving).into(),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("orphan physical port");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
        .push(Op::Connect {
            from: relation.erase(),
            to: orphan.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView { view: view.into() });
    let messages = diagnostics(transaction, model);
    assert!(
        messages
            .iter()
            .any(|message| message.contains("owning Relation"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Connection membership"))
    );
}
