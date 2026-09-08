use eqiora_core::{DimExponents, Id, ScalarDomain, ValueLiteral, ValueType, entity::kinds};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Precondition, Transaction};
use eqiora_schema::kernel::ParameterDef;

#[test]
fn typed_edits_preserve_imaginary_components_snapshots_and_atomic_preconditions() {
    let kind = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
        .expect("valid scalar type")
        .array(2)
        .unwrap();
    let before = ValueLiteral::new(kind.clone(), [(2.0, 3.0), (5.0, 7.0)]).unwrap();
    let after = ValueLiteral::new(kind.clone(), [(2.0, 11.0), (5.0, 13.0)]).unwrap();
    let id = Id::<kinds::Parameter>::new();
    let mut store = InMemoryGraphStore::new();
    let mut create = Transaction::new("typed channels");
    create.push(Op::DefineKernelNode {
        node: ParameterDef::new(id, before.clone()).into(),
    });
    store.commit(create).unwrap();
    let original = store.snapshot();
    let mut update = Transaction::new("imaginary-only edit");
    update.require(Precondition::ValueEquals {
        target: id.erase(),
        expected: before.clone(),
    });
    update.push(Op::SetValue {
        target: id.erase(),
        value: after.clone(),
    });
    store.commit(update).unwrap();
    assert_eq!(original.node(id.erase()).unwrap().value(), Some(&before));
    assert_eq!(
        store.snapshot().node(id.erase()).unwrap().value(),
        Some(&after)
    );

    // Real parts alone cannot satisfy a stale optimistic precondition.
    let mut stale = Transaction::new("stale imaginary components");
    stale.require(Precondition::ValueEquals {
        target: id.erase(),
        expected: before.clone(),
    });
    stale.push(Op::SetValue {
        target: id.erase(),
        value: before.clone(),
    });
    assert!(store.commit(stale).is_err());

    // A later shape-changing operation rolls back the earlier valid edit too.
    let scalar = ValueLiteral::new(
        ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
            .expect("valid scalar type"),
        [(2.0, 3.0)],
    )
    .unwrap();
    let mut invalid = Transaction::new("invalid type change");
    invalid.push(Op::SetValue {
        target: id.erase(),
        value: before,
    });
    invalid.push(Op::SetValue {
        target: id.erase(),
        value: scalar,
    });
    assert!(store.commit(invalid).is_err());
    assert_eq!(
        store.snapshot().node(id.erase()).unwrap().value(),
        Some(&after)
    );
}

#[test]
fn adjacent_large_integers_remain_distinct_across_edits_and_preconditions() {
    let ty = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS)
        .expect("valid scalar type");
    let before = ValueLiteral::from_integer(ty.clone(), 9_007_199_254_740_992).unwrap();
    let after = ValueLiteral::from_integer(ty, 9_007_199_254_740_993).unwrap();
    let id = Id::<kinds::Parameter>::new();
    let mut store = InMemoryGraphStore::new();
    let mut create = Transaction::new("exact integer");
    create.push(Op::DefineKernelNode {
        node: ParameterDef::new(id, before.clone()).into(),
    });
    store.commit(create).unwrap();
    let snapshot = store.snapshot();
    let mut edit = Transaction::new("adjacent integer");
    edit.require(Precondition::ValueEquals {
        target: id.erase(),
        expected: before.clone(),
    });
    edit.push(Op::SetValue {
        target: id.erase(),
        value: after.clone(),
    });
    store.commit(edit).unwrap();
    assert_eq!(snapshot.node(id.erase()).unwrap().value(), Some(&before));
    assert_eq!(
        store.snapshot().node(id.erase()).unwrap().value(),
        Some(&after)
    );
    let revision = store.snapshot().revision();
    let mut stale = Transaction::new("rounded equality is not exact equality");
    stale.require(Precondition::ValueEquals {
        target: id.erase(),
        expected: before.clone(),
    });
    stale.push(Op::SetValue {
        target: id.erase(),
        value: before,
    });
    assert!(store.commit(stale).is_err());
    assert_eq!(store.snapshot().revision(), revision);
    assert_eq!(
        store.snapshot().node(id.erase()).unwrap().value(),
        Some(&after)
    );
}

#[test]
fn structural_value_guard_survives_dependency_owner_removal_and_recreation() {
    use eqiora_graph::{EdgeKind, Revision};
    use eqiora_schema::kernel::{IndexSetDef, KernelNode};

    let parameter = Id::<kinds::Parameter>::new();
    let set = Id::<kinds::IndexSet>::new();
    let ty = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS)
        .expect("valid scalar type");
    let before = ValueLiteral::from_integer(ty.clone(), 3).unwrap();
    let after = ValueLiteral::from_integer(ty, 4).unwrap();
    let definition: KernelNode = IndexSetDef::new(set, 3).unwrap().into();
    let mut hydration = Transaction::new("complete structural snapshot");
    hydration.push(Op::DefineKernelNode {
        node: ParameterDef::new(parameter, before.clone()).into(),
    });
    hydration.push(Op::SetValue {
        target: parameter.erase(),
        value: before.clone(),
    });
    hydration.push(Op::DefineKernelNode {
        node: definition.clone(),
    });
    hydration.push(Op::Connect {
        from: set.erase(),
        to: parameter.erase(),
        edge: EdgeKind::DependsOn,
    });
    let mut store = InMemoryGraphStore::restore_snapshot(hydration, Revision(7)).unwrap();

    let mut no_op = Transaction::new("unchanged structural value");
    no_op.push(Op::SetValue {
        target: parameter.erase(),
        value: before.clone(),
    });
    assert!(store.validate(&no_op).is_empty());
    store.commit(no_op).unwrap();
    let original = store.snapshot();

    let mut bypass = Transaction::new("temporarily remove structural dependency");
    bypass.push(Op::RemoveNode { id: set.erase() });
    bypass.push(Op::SetValue {
        target: parameter.erase(),
        value: after,
    });
    bypass.push(Op::DefineKernelNode {
        node: definition.clone(),
    });
    bypass.push(Op::Connect {
        from: set.erase(),
        to: parameter.erase(),
        edge: EdgeKind::DependsOn,
    });
    let diagnostics = store.validate(&bypass);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("determines an IndexSet"));
    assert!(store.commit(bypass).is_err());
    let unchanged = store.snapshot();
    assert_eq!(unchanged.revision(), original.revision());
    assert_eq!(
        unchanged.node(parameter.erase()).unwrap().value(),
        Some(&before)
    );
    assert_eq!(
        unchanged.node(set.erase()).unwrap().kernel_definition(),
        Some(&definition)
    );
    assert_eq!(
        unchanged.edges().collect::<Vec<_>>(),
        original.edges().collect::<Vec<_>>()
    );
}
