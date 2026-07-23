use eqiora_core::entity::kinds;
use eqiora_core::quantity::dim;
use eqiora_core::{DimExponents, Dimension, DynQuantity, EntityKind, Id, OntologyId, ValueShape};
use eqiora_graph::{
    EdgeKind, GraphStore, InMemoryGraphStore, Op, Precondition, Revision, Transaction,
};
use eqiora_schema::kernel::{
    ExprDagBuilder, FieldDef, KernelNode, ParameterDef, PortDef, RelationDef, SignalDirection,
    ValueFrame,
};
use eqiora_schema::{Model, ModelView};

fn define(definition: impl Into<KernelNode>) -> Op {
    Op::DefineKernelNode {
        node: definition.into(),
    }
}

fn zero_relation(id: Id<kinds::Relation>) -> RelationDef {
    let mut expressions = ExprDagBuilder::new();
    let zero = expressions
        .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
        .expect("one constant fits the arena");
    RelationDef::new(id, expressions.finish([zero]).expect("one residual root"))
}

#[test]
fn commit_is_atomic_and_records_provenance() {
    let field = Id::<kinds::Field>::new();
    let mut transaction = Transaction::new("add inlet velocity");
    transaction
        .push(define(FieldDef::new(field, dim::VelocityDim::EXPONENTS)))
        .push(Op::SetValue {
            target: field.erase(),
            value: DynQuantity::new(12.0, dim::VelocityDim::EXPONENTS),
        });

    let mut store = InMemoryGraphStore::new();
    let committed = store.commit(transaction).expect("valid transaction");
    let snapshot = store.snapshot();

    assert_eq!(committed.revision, Revision(1));
    assert_eq!(snapshot.revision(), Revision(1));
    assert_eq!(
        snapshot.node(field.erase()).and_then(|node| node.value()),
        Some(DynQuantity::new(12.0, dim::VelocityDim::EXPONENTS))
    );
    assert!(snapshot.node(committed.transaction.erase()).is_some());
    assert_eq!(snapshot.commits().len(), 1);
    assert_eq!(snapshot.commits()[0].label(), "add inlet velocity");
}

#[test]
fn failed_operation_rolls_back_the_whole_transaction() {
    let field = Id::<kinds::Field>::new();
    let domain = Id::<kinds::Domain>::new();
    let mut transaction = Transaction::new("invalid mixed transaction");
    transaction
        .push(define(FieldDef::new(field, dim::VelocityDim::EXPONENTS)))
        .push(Op::SetValue {
            target: domain.erase(),
            value: DynQuantity::new(1.0, dim::LengthDim::EXPONENTS),
        });

    let mut store = InMemoryGraphStore::new();
    assert!(store.commit(transaction).is_err());
    assert_eq!(store.revision(), Revision(0));
    assert!(store.snapshot().node(field.erase()).is_none());
    assert!(store.snapshot().commits().is_empty());
}

#[test]
fn optimistic_preconditions_preserve_snapshot_isolation() {
    let parameter = Id::<kinds::Parameter>::new();
    let initial = DynQuantity::new(12.0, dim::VelocityDim::EXPONENTS);
    let mut add = Transaction::new("add parameter");
    add.push(define(ParameterDef::new(parameter, initial)));

    let mut store = InMemoryGraphStore::new();
    store.commit(add).expect("setup succeeds");
    let old_snapshot = store.snapshot();

    let mut update = Transaction::new("update parameter");
    update
        .require(Precondition::RevisionIs(Revision(1)))
        .require(Precondition::ValueEquals {
            target: parameter.erase(),
            expected: initial,
        })
        .push(Op::SetValue {
            target: parameter.erase(),
            value: DynQuantity::new(15.0, dim::VelocityDim::EXPONENTS),
        });
    store.commit(update).expect("preconditions match");

    assert_eq!(old_snapshot.revision(), Revision(1));
    assert_eq!(
        old_snapshot
            .node(parameter.erase())
            .and_then(|node| node.value()),
        Some(initial)
    );
    assert_eq!(store.revision(), Revision(2));
}

#[test]
fn restored_snapshot_retains_its_revision_and_advances_normally() {
    let parameter = Id::<kinds::Parameter>::new();
    let initial = DynQuantity::new(12.0, dim::VelocityDim::EXPONENTS);
    let mut snapshot = Transaction::new("restore complete snapshot");
    snapshot.push(define(ParameterDef::new(parameter, initial)));

    let mut store = InMemoryGraphStore::restore_snapshot(snapshot, Revision(7))
        .expect("a complete snapshot can be hydrated at its recorded revision");
    let restored = store.snapshot();
    assert_eq!(restored.revision(), Revision(7));
    assert!(
        restored.commits().is_empty(),
        "hydration must not fabricate unavailable commit history"
    );
    assert_eq!(
        restored
            .node(parameter.erase())
            .and_then(|node| node.value()),
        Some(initial)
    );

    let mut update = Transaction::new("advance restored snapshot");
    update
        .require(Precondition::RevisionIs(Revision(7)))
        .require(Precondition::ValueEquals {
            target: parameter.erase(),
            expected: initial,
        })
        .push(Op::SetValue {
            target: parameter.erase(),
            value: DynQuantity::new(15.0, dim::VelocityDim::EXPONENTS),
        });
    let committed = store
        .commit(update)
        .expect("ordinary commit follows restoration");
    assert_eq!(committed.revision, Revision(8));
    assert_eq!(store.revision(), Revision(8));
    assert_eq!(store.snapshot().commits().len(), 1);
    assert_eq!(store.snapshot().commits()[0].revision(), Revision(8));
}

#[test]
fn snapshot_restoration_rejects_zero_revision_and_preconditions() {
    let empty = Transaction::new("invalid zero-revision restoration");
    let diagnostics = InMemoryGraphStore::restore_snapshot(empty, Revision(0))
        .expect_err("revision zero describes only the pristine store");
    assert_eq!(diagnostics[0].code().0, "EQ0105");

    let mut conditional = Transaction::new("invalid conditional restoration");
    conditional.require(Precondition::RevisionIs(Revision(7)));
    let diagnostics = InMemoryGraphStore::restore_snapshot(conditional, Revision(7))
        .expect_err("snapshot hydration cannot depend on missing prior state");
    assert_eq!(diagnostics[0].code().0, "EQ0105");
}

#[test]
fn dimension_change_is_rejected() {
    let parameter = Id::<kinds::Parameter>::new();
    let mut setup = Transaction::new("add length");
    setup.push(define(ParameterDef::new(
        parameter,
        DynQuantity::new(2.0, dim::LengthDim::EXPONENTS),
    )));
    let mut store = InMemoryGraphStore::new();
    store.commit(setup).expect("setup succeeds");

    let mut invalid = Transaction::new("change dimension");
    invalid.push(Op::SetValue {
        target: parameter.erase(),
        value: DynQuantity::new(2.0, dim::TimeDim::EXPONENTS),
    });
    let diagnostics = store.commit(invalid).expect_err("dimension must be stable");

    assert_eq!(diagnostics[0].code().0, "EQ0401");
    assert_eq!(store.revision(), Revision(1));
}

#[test]
fn scalar_set_value_cannot_initialize_a_shaped_field() {
    let field = Id::<kinds::Field>::new();
    let definition = FieldDef::shaped(
        field,
        dim::VelocityDim::EXPONENTS,
        ValueShape::new([2]).unwrap(),
        ValueFrame::SpatialCartesian,
    )
    .unwrap();
    let mut transaction = Transaction::new("reject scalar shaped-field value");
    transaction.push(define(definition)).push(Op::SetValue {
        target: field.erase(),
        value: DynQuantity::new(1.0, dim::VelocityDim::EXPONENTS),
    });

    let mut store = InMemoryGraphStore::new();
    let diagnostics = store
        .commit(transaction)
        .expect_err("shaped Field needs a future shaped-value contract");
    assert_eq!(diagnostics[0].code().0, "EQ0105");
    assert_eq!(store.revision(), Revision(0));
}

#[test]
fn graph_boundaries_are_checked_by_edge_kind() {
    let space = Id::<kinds::Space>::new();
    let field = Id::<kinds::Field>::new();
    let mut valid = Transaction::new("discretize field");
    valid
        .push(Op::AddNode {
            kind: EntityKind::Space,
            id: space.erase(),
        })
        .push(define(FieldDef::new(field, dim::LengthDim::EXPONENTS)))
        .push(Op::Connect {
            from: space.erase(),
            to: field.erase(),
            edge: EdgeKind::Discretizes,
        });
    let mut store = InMemoryGraphStore::new();
    store.commit(valid).expect("schema-approved cross edge");
    assert_eq!(store.snapshot().edges().len(), 1);

    let evidence = Id::<kinds::Evidence>::new();
    let mut invalid = Transaction::new("misuse discretizes");
    invalid
        .push(Op::AddNode {
            kind: EntityKind::Evidence,
            id: evidence.erase(),
        })
        .push(Op::Connect {
            from: evidence.erase(),
            to: field.erase(),
            edge: EdgeKind::Discretizes,
        });
    assert!(store.commit(invalid).is_err());
    assert!(store.snapshot().node(evidence.erase()).is_none());
}

#[test]
fn erased_id_kind_mismatch_is_rejected() {
    let field = Id::<kinds::Field>::new();
    let mut transaction = Transaction::new("lie about ID kind");
    transaction.push(Op::AddNode {
        kind: EntityKind::Parameter,
        id: field.erase(),
    });

    let diagnostics = InMemoryGraphStore::new().validate(&transaction);
    assert_eq!(diagnostics[0].code().0, "EQ0103");
}

#[test]
fn ontology_view_commits_with_its_kernel_members_but_is_not_a_node() {
    let relation = Id::<kinds::Relation>::new();
    let port = Id::<kinds::Port>::new();
    let model = ModelView::new(
        OntologyId::new(),
        [relation.erase(), port.erase()],
        [port.erase()],
    )
    .expect("valid model view");
    let model_id = model.id().erase();

    let mut transaction = Transaction::new("define model and members atomically");
    transaction
        .push(Op::DefineOntologyView { view: model.into() })
        .push(define(zero_relation(relation)))
        .push(define(PortDef::signal(
            port,
            SignalDirection::Input,
            DimExponents::DIMENSIONLESS,
        )));

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("one atomic revision");
    let snapshot = store.snapshot();
    let erased = snapshot
        .ontology_view(&model_id)
        .expect("view is registered beside graph nodes");

    assert_eq!(snapshot.nodes().len(), 3); // two kernel nodes + provenance
    assert_eq!(snapshot.ontology_views().len(), 1);
    assert_eq!(
        erased
            .downcast::<Model>()
            .expect("schema matches")
            .id()
            .erase(),
        model_id
    );
}

#[test]
fn view_cannot_reference_a_node_outside_its_revision() {
    let relation = Id::<kinds::Relation>::new();
    let model = ModelView::new(OntologyId::new(), [relation.erase()], [])
        .expect("structurally valid before store lookup");
    let mut transaction = Transaction::new("dangling ontology view");
    transaction.push(Op::DefineOntologyView { view: model.into() });

    let mut store = InMemoryGraphStore::new();
    let diagnostics = store
        .commit(transaction)
        .expect_err("missing member must reject the whole commit");

    assert_eq!(diagnostics[0].code().0, "EQ0101");
    assert_eq!(store.revision(), Revision(0));
    assert_eq!(store.snapshot().ontology_views().len(), 0);
}

#[test]
fn referenced_node_requires_explicit_view_removal() {
    let relation = Id::<kinds::Relation>::new();
    let model =
        ModelView::new(OntologyId::new(), [relation.erase()], []).expect("valid model view");
    let model_id = model.id().erase();
    let mut setup = Transaction::new("setup model");
    setup
        .push(define(zero_relation(relation)))
        .push(Op::DefineOntologyView { view: model.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(setup).expect("setup succeeds");

    let mut invalid = Transaction::new("remove a referenced member");
    invalid.push(Op::RemoveNode {
        id: relation.erase(),
    });
    let diagnostics = store.commit(invalid).expect_err("a view must never dangle");
    assert_eq!(diagnostics[0].code().0, "EQ0204");
    assert!(store.snapshot().node(relation.erase()).is_some());

    let mut explicit = Transaction::new("remove model and member atomically");
    explicit
        .push(Op::RemoveNode {
            id: relation.erase(),
        })
        .push(Op::RemoveOntologyView {
            id: model_id.clone(),
        });
    store
        .commit(explicit)
        .expect("referential integrity is checked on the final atomic state");

    let snapshot = store.snapshot();
    assert!(snapshot.ontology_view(&model_id).is_none());
    assert!(snapshot.node(relation.erase()).is_none());
}

#[test]
fn raw_semantic_node_creation_is_rejected() {
    let field = Id::<kinds::Field>::new();
    let mut transaction = Transaction::new("incomplete semantic node");
    transaction.push(Op::AddNode {
        kind: EntityKind::Field,
        id: field.erase(),
    });

    let diagnostics = InMemoryGraphStore::new().validate(&transaction);
    assert_eq!(diagnostics[0].code().0, "EQ0105");
}
