use eqiora_core::{DimExponents, Id, ScalarDomain, ValueLiteral, ValueType, entity::kinds};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Precondition, Transaction};
use eqiora_schema::kernel::ParameterDef;

#[test]
fn typed_edits_preserve_imaginary_components_snapshots_and_atomic_preconditions() {
    let kind = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
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
        ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS),
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
