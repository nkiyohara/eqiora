use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId, RawId, ValueShape};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, AxisBounds, BoundarySide, ClockDomainDef, ConnectionDef,
    ConnectionSemantics, DomainDef, ExprDagBuilder, ExprId, FieldDef, KernelNode, ParameterDef,
    PortDef, RationalTime, RelationDef, RepresentationDef, SignalDirection, SymbolRef, ValueFrame,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;

#[test]
fn valid_program_owns_one_snapshot_revision() {
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();

    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(SymbolRef::Field(field)).expect("field");
    let zero = expression
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .expect("zero");
    let residual = expression.sub(value, zero).expect("residual");

    let mut transaction = Transaction::new("valid continuous model");
    for node in [
        KernelNode::from(
            FieldDef::new(field, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                .expect("initial value"),
        ),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
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
        .push(Op::DefineOntologyView {
            view: ModelView::new(
                model,
                [field.erase(), relation.erase(), activation.erase()],
                [],
            )
            .expect("ModelView")
            .into(),
        });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("valid graph");
    let snapshot = store.snapshot();
    let program = KernelProgram::from_snapshot(&snapshot, model).expect("valid program");

    let mut later = Transaction::new("unrelated later commit");
    later.push(Op::SetValue {
        target: field.erase(),
        value: DynQuantity::new(2.0, DimExponents::DIMENSIONLESS),
    });
    store.commit(later).expect("later commit");
    let later_program =
        KernelProgram::from_snapshot(&store.snapshot(), model).expect("new revision");

    assert_eq!(program.revision(), snapshot.revision());
    assert_ne!(program.revision(), store.snapshot().revision());
    assert_eq!(program.model(), model);
    assert_eq!(program.nodes().len(), 3);
    assert_eq!(program.edges().len(), 2);
    assert_eq!(
        program
            .value(field.erase())
            .expect("captured value")
            .value(),
        1.0
    );
    assert_eq!(
        later_program
            .value(field.erase())
            .expect("new value")
            .value(),
        2.0
    );
}

#[test]
fn symbol_outside_model_is_rejected() {
    let external_field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();

    let mut expression = ExprDagBuilder::new();
    let residual = expression
        .symbol(SymbolRef::Field(external_field))
        .expect("field");
    let mut transaction = Transaction::new("model with external symbol");
    for node in [
        KernelNode::from(FieldDef::new(external_field, DimExponents::DIMENSIONLESS)),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
        .push(Op::Connect {
            from: relation.erase(),
            to: external_field.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView {
            view: ModelView::new(model, [relation.erase(), activation.erase()], [])
                .expect("ModelView")
                .into(),
        });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    let diagnostics = KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect_err("model selection is not closed");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == codes::UNRESOLVED_SYMBOL)
    );
}

#[test]
fn incompatible_expression_dimensions_are_rejected() {
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();
    let time_dimension =
        DimExponents::from_integers([0, 0, 1, 0, 0, 0, 0]).expect("bounded dimension");

    let mut expression = ExprDagBuilder::new();
    let time = expression.symbol(SymbolRef::Field(field)).expect("field");
    let dimensionless = expression
        .constant(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
        .expect("constant");
    let residual = expression.sub(time, dimensionless).expect("structural DAG");
    let mut transaction = Transaction::new("dimensionally invalid model");
    for node in [
        KernelNode::from(FieldDef::new(field, time_dimension)),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
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
        .push(Op::DefineOntologyView {
            view: ModelView::new(
                model,
                [field.erase(), relation.erase(), activation.erase()],
                [],
            )
            .expect("ModelView")
            .into(),
        });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    let diagnostics =
        KernelProgram::from_snapshot(&store.snapshot(), model).expect_err("dimensions conflict");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::INVALID_RELATION_DIMENSION
            && diagnostic
                .graph_path()
                .is_some_and(|path| path.to_string().contains("expression"))
    }));
}

#[test]
fn shaped_relation_roots_are_componentwise_but_activation_roots_remain_scalar() {
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();
    let shape = ValueShape::new([2]).unwrap();

    let mut residual = ExprDagBuilder::new();
    let residual_root = residual.symbol(SymbolRef::Field(field)).unwrap();
    let mut guard = ExprDagBuilder::new();
    let guard_root = guard.symbol(SymbolRef::Field(field)).unwrap();
    let activation_definition = ActivationDef::new(
        activation,
        ActivationKind::Guard {
            guard: guard.finish([guard_root]).unwrap(),
        },
    )
    .unwrap();

    let nodes = [
        KernelNode::from(
            FieldDef::shaped(
                field,
                DimExponents::DIMENSIONLESS,
                shape,
                ValueFrame::Invariant,
            )
            .unwrap(),
        ),
        KernelNode::from(RelationDef::new(
            relation,
            residual.finish([residual_root]).unwrap(),
        )),
        KernelNode::from(activation_definition),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("shaped activation guard falsifier");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
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
        .push(Op::DefineOntologyView { view: view.into() });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let diagnostics = KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect_err("activation Guard cannot consume a shaped root");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("expression root must be an invariant scalar")
    }));
}

#[test]
fn boundary_operator_without_boundary_scope_is_rejected() {
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();

    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(SymbolRef::Field(field)).expect("field");
    let residual = expression.trace(value).expect("trace node");
    let mut transaction = Transaction::new("unscoped boundary operator");
    for node in [
        KernelNode::from(FieldDef::new(field, DimExponents::DIMENSIONLESS)),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
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
        .push(Op::DefineOntologyView {
            view: ModelView::new(
                model,
                [field.erase(), relation.erase(), activation.erase()],
                [],
            )
            .expect("ModelView")
            .into(),
        });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    let diagnostics = KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect_err("trace requires a boundary Relation scope");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::INVALID_KERNEL_DEFINITION
            && diagnostic.message().contains("boundary Domain")
    }));
}

#[test]
fn derivative_dimension_overflow_is_not_misreported_as_missing_symbol() {
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();
    let extreme_dimension = DimExponents::from_integers([0, 0, -i32::MAX, 0, 0, 0, 0])
        .expect("lowest admitted integer dimension exponent");

    let mut expression = ExprDagBuilder::new();
    let residual = expression
        .symbol(SymbolRef::Derivative(field))
        .expect("derivative");
    let mut transaction = Transaction::new("derivative dimension overflow");
    for node in [
        KernelNode::from(FieldDef::new(field, extreme_dimension)),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
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
        .push(Op::DefineOntologyView {
            view: ModelView::new(
                model,
                [field.erase(), relation.erase(), activation.erase()],
                [],
            )
            .expect("ModelView")
            .into(),
        });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    let diagnostics = KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect_err("dimension exponent overflows");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == codes::INVALID_RELATION_DIMENSION)
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code() != codes::UNRESOLVED_SYMBOL)
    );
}

#[test]
fn periodic_activation_requires_one_periodic_clock() {
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let clock = Id::<kinds::ClockDomain>::new();
    let model = OntologyId::<Model>::new();

    let mut expression = ExprDagBuilder::new();
    let residual = expression
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .expect("constant");
    let mut transaction = Transaction::new("periodic model missing clock edge");
    for node in [
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::periodic(activation)),
        KernelNode::from(
            ClockDomainDef::periodic(
                clock,
                RationalTime::new(1, 100).expect("period"),
                RationalTime::ZERO,
            )
            .expect("periodic clock"),
        ),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView {
            view: ModelView::new(
                model,
                [relation.erase(), activation.erase(), clock.erase()],
                [],
            )
            .expect("ModelView")
            .into(),
        });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    let diagnostics =
        KernelProgram::from_snapshot(&store.snapshot(), model).expect_err("clock edge is required");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == codes::INVALID_CLOCK)
    );
}

#[test]
fn signal_connection_supports_one_to_many_fanout() {
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let connection = Id::<kinds::Connection>::new();
    let output = Id::<kinds::Port>::new();
    let input_a = Id::<kinds::Port>::new();
    let input_b = Id::<kinds::Port>::new();
    let model = OntologyId::<Model>::new();

    let mut expression = ExprDagBuilder::new();
    let residual = expression
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .expect("constant");
    let mut transaction = Transaction::new("signal fanout model");
    for node in [
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
        KernelNode::from(ConnectionDef::new(connection, ConnectionSemantics::Signal)),
        KernelNode::from(PortDef::signal(
            output,
            SignalDirection::Output,
            DimExponents::DIMENSIONLESS,
        )),
        KernelNode::from(PortDef::signal(
            input_a,
            SignalDirection::Input,
            DimExponents::DIMENSIONLESS,
        )),
        KernelNode::from(PortDef::signal(
            input_b,
            SignalDirection::Input,
            DimExponents::DIMENSIONLESS,
        )),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction.push(Op::Connect {
        from: activation.erase(),
        to: relation.erase(),
        edge: EdgeKind::Activates,
    });
    for port in [output, input_a, input_b] {
        transaction.push(Op::Connect {
            from: connection.erase(),
            to: port.erase(),
            edge: EdgeKind::Connects,
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(
            model,
            [
                relation.erase(),
                activation.erase(),
                connection.erase(),
                output.erase(),
                input_a.erase(),
                input_b.erase(),
            ],
            [output.erase(), input_a.erase(), input_b.erase()],
        )
        .expect("ModelView")
        .into(),
    });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("valid graph");
    let program = KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect("signal fanout has one causal source");

    assert_eq!(program.boundary().len(), 3);
}

#[test]
fn semantic_validation_consumes_the_shared_scalar_connection_contract() {
    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");
    let dimensions = invalid_signal_connection([
        (SignalDirection::Output, DimExponents::DIMENSIONLESS),
        (SignalDirection::Input, length),
    ]);
    assert!(
        dimensions.iter().any(|diagnostic| {
            diagnostic.code() == codes::INVALID_RELATION_DIMENSION
                && diagnostic.graph_path().is_some()
                && diagnostic
                    .message()
                    .contains("identical physical dimensions")
        }),
        "{dimensions:?}"
    );

    let directions = invalid_signal_connection([
        (SignalDirection::Output, DimExponents::DIMENSIONLESS),
        (SignalDirection::Output, DimExponents::DIMENSIONLESS),
    ]);
    assert!(directions.iter().any(|diagnostic| {
        diagnostic.code() == codes::INVALID_KERNEL_DEFINITION
            && diagnostic.graph_path().is_some()
            && diagnostic.message().contains("found 2 outputs")
    }));
}

fn invalid_signal_connection(
    ports: [(SignalDirection, DimExponents); 2],
) -> Vec<eqiora_core::Diagnostic> {
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let connection = Id::<kinds::Connection>::new();
    let port_ids = [Id::<kinds::Port>::new(), Id::<kinds::Port>::new()];
    let model = OntologyId::<Model>::new();
    let mut expression = ExprDagBuilder::new();
    let residual = expression
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .expect("constant");
    let mut transaction = Transaction::new("invalid signal compatibility");
    for node in [
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
        KernelNode::from(ConnectionDef::new(connection, ConnectionSemantics::Signal)),
        KernelNode::from(PortDef::signal(port_ids[0], ports[0].0, ports[0].1)),
        KernelNode::from(PortDef::signal(port_ids[1], ports[1].0, ports[1].1)),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction.push(Op::Connect {
        from: activation.erase(),
        to: relation.erase(),
        edge: EdgeKind::Activates,
    });
    for port in port_ids {
        transaction.push(Op::Connect {
            from: connection.erase(),
            to: port.erase(),
            edge: EdgeKind::Connects,
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(
            model,
            [
                relation.erase(),
                activation.erase(),
                connection.erase(),
                port_ids[0].erase(),
                port_ids[1].erase(),
            ],
            [],
        )
        .expect("ModelView")
        .into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect_err("signal contract is invalid")
}

#[test]
fn one_port_cannot_belong_to_two_connection_nets() {
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let connection_a = Id::<kinds::Connection>::new();
    let connection_b = Id::<kinds::Connection>::new();
    let output_a = Id::<kinds::Port>::new();
    let output_b = Id::<kinds::Port>::new();
    let shared_input = Id::<kinds::Port>::new();
    let model = OntologyId::<Model>::new();

    let mut expression = ExprDagBuilder::new();
    let residual = expression
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .expect("constant");
    let mut transaction = Transaction::new("ambiguous signal input");
    for node in [
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
        KernelNode::from(ConnectionDef::new(
            connection_a,
            ConnectionSemantics::Signal,
        )),
        KernelNode::from(ConnectionDef::new(
            connection_b,
            ConnectionSemantics::Signal,
        )),
        KernelNode::from(PortDef::signal(
            output_a,
            SignalDirection::Output,
            DimExponents::DIMENSIONLESS,
        )),
        KernelNode::from(PortDef::signal(
            output_b,
            SignalDirection::Output,
            DimExponents::DIMENSIONLESS,
        )),
        KernelNode::from(PortDef::signal(
            shared_input,
            SignalDirection::Input,
            DimExponents::DIMENSIONLESS,
        )),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction.push(Op::Connect {
        from: activation.erase(),
        to: relation.erase(),
        edge: EdgeKind::Activates,
    });
    for (connection, output) in [(connection_a, output_a), (connection_b, output_b)] {
        for port in [output, shared_input] {
            transaction.push(Op::Connect {
                from: connection.erase(),
                to: port.erase(),
                edge: EdgeKind::Connects,
            });
        }
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(
            model,
            [
                relation.erase(),
                activation.erase(),
                connection_a.erase(),
                connection_b.erase(),
                output_a.erase(),
                output_b.erase(),
                shared_input.erase(),
            ],
            [],
        )
        .expect("ModelView")
        .into(),
    });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    let diagnostics = KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect_err("connection ownership is ambiguous");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::INVALID_KERNEL_DEFINITION
            && diagnostic.message().contains("belongs to both Connection")
    }));
}

#[derive(Clone, Copy)]
struct SpatialIds {
    body: Id<kinds::Domain>,
    other: Id<kinds::Domain>,
    wall: Id<kinds::Domain>,
    field: Id<kinds::Field>,
    other_field: Id<kinds::Field>,
    parameter: Id<kinds::Parameter>,
}

fn invalid_spatial_expression(
    scoped: bool,
    dependencies: &[RawId],
    build: impl FnOnce(&mut ExprDagBuilder, SpatialIds) -> ExprId,
) -> Vec<eqiora_core::Diagnostic> {
    let ids = SpatialIds {
        body: Id::new(),
        other: Id::new(),
        wall: Id::new(),
        field: Id::new(),
        other_field: Id::new(),
        parameter: Id::new(),
    };
    let representation = Id::<kinds::Representation>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();
    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");
    let bounds = || {
        vec![
            AxisBounds::new(DynQuantity::new(0.0, length), DynQuantity::new(1.0, length))
                .expect("axis"),
        ]
    };
    let mut expression = ExprDagBuilder::new();
    let residual = build(&mut expression, ids);
    let nodes = [
        KernelNode::from(DomainDef::cartesian_box(ids.body, bounds()).expect("body")),
        KernelNode::from(DomainDef::cartesian_box(ids.other, bounds()).expect("other")),
        KernelNode::from(DomainDef::cartesian_boundary(
            ids.wall,
            0,
            BoundarySide::Lower,
        )),
        KernelNode::from(RepresentationDef::continuum(representation)),
        KernelNode::from(FieldDef::new(ids.field, DimExponents::DIMENSIONLESS)),
        KernelNode::from(FieldDef::new(ids.other_field, DimExponents::DIMENSIONLESS)),
        KernelNode::from(ParameterDef::new(
            ids.parameter,
            DynQuantity::new(1.0, DimExponents::DIMENSIONLESS),
        )),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([residual]).expect("DAG"),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("spatial typing falsifier");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (field, domain) in [(ids.field, ids.body), (ids.other_field, ids.other)] {
        transaction
            .push(Op::Connect {
                from: field.erase(),
                to: domain.erase(),
                edge: EdgeKind::DefinedOn,
            })
            .push(Op::Connect {
                from: field.erase(),
                to: representation.erase(),
                edge: EdgeKind::DefinedOn,
            });
    }
    transaction
        .push(Op::Connect {
            from: ids.wall.erase(),
            to: ids.body.erase(),
            edge: EdgeKind::BoundaryOf,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        });
    if scoped {
        transaction.push(Op::Connect {
            from: relation.erase(),
            to: ids.body.erase(),
            edge: EdgeKind::AppliesOn,
        });
    }
    for dependency in dependencies {
        transaction.push(Op::Connect {
            from: relation.erase(),
            to: *dependency,
            edge: EdgeKind::DependsOn,
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).expect("view").into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("graph-local validity");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect_err("typing must fail")
}

#[test]
fn shared_spatial_contract_falsifiers_reach_graph_diagnostics() {
    let cases = [
        (
            "grad parameter",
            "gradient operand has no spatial Domain support",
            invalid_spatial_expression(false, &[], |expression, ids| {
                let parameter = expression
                    .symbol(SymbolRef::Parameter(ids.parameter))
                    .expect("parameter");
                expression.gradient(parameter).expect("gradient")
            }),
        ),
        (
            "div scalar",
            "divergence requires a spatial tensor operand",
            invalid_spatial_expression(true, &[], |expression, ids| {
                let field = expression
                    .symbol(SymbolRef::Field(ids.field))
                    .expect("field");
                expression.divergence(field).expect("divergence")
            }),
        ),
        (
            "symmetric scalar",
            "symmetric_part requires an exact [d,d] spatial Cartesian tensor",
            invalid_spatial_expression(true, &[], |expression, ids| {
                let field = expression
                    .symbol(SymbolRef::Field(ids.field))
                    .expect("field");
                expression.symmetric_part(field).expect("symmetric part")
            }),
        ),
        (
            "lift parameter",
            "isotropic_lift requires a Cartesian volume operand",
            invalid_spatial_expression(false, &[], |expression, ids| {
                let parameter = expression
                    .symbol(SymbolRef::Parameter(ids.parameter))
                    .expect("parameter");
                expression
                    .isotropic_lift(parameter)
                    .expect("isotropic lift")
            }),
        ),
    ];
    for (name, message, diagnostics) in cases {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(message)),
            "{name}: {diagnostics:?}"
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .graph_path()
                .is_some_and(|path| path.to_string().contains("expression"))
        }));
    }
}
