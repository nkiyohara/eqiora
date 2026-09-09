//! Closed record membership and shared temporal ownership during Model admission.
use super::*;

pub(super) fn validate(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&owner, node) in nodes {
        match node {
            KernelNode::Record(record) => {
                for (_, value_type) in record.members() {
                    super::nominal_values::check(owner, value_type, nodes, diagnostics);
                }
            }
            KernelNode::RecordInstance(instance) => {
                let Some(KernelNode::Record(record)) = nodes.get(&instance.definition().erase())
                else {
                    diagnostics.push(kernel_error(
                        owner,
                        "record instance requires its exact selected Record declaration",
                    ));
                    continue;
                };
                if instance.members().len() != record.members().len() {
                    diagnostics.push(kernel_error(
                        owner,
                        "record instance must bind every declared member exactly once",
                    ));
                    continue;
                }
                let mut temporal_owner = None;
                for ((_, expected_type), member) in record.members().iter().zip(instance.members())
                {
                    let (value_type, temporal) = match nodes.get(member) {
                        Some(KernelNode::Parameter(parameter)) => {
                            (parameter.value_type(), (None, BTreeSet::new()))
                        }
                        Some(KernelNode::Field(field)) => (
                            field.value_type(),
                            (
                                Some(field.role()),
                                edge_targets(edges, *member, EdgeKind::ClockedBy),
                            ),
                        ),
                        _ => {
                            diagnostics.push(kernel_error(
                                owner,
                                "record member requires its exact selected Field or Parameter",
                            ));
                            continue;
                        }
                    };
                    if value_type != expected_type {
                        diagnostics.push(kernel_error(owner, "record member disagrees with its declared type or ordered member identity"));
                    }
                    if temporal.1.len() > 1 || temporal.1.iter().any(|clock| !matches!(nodes.get(clock), Some(KernelNode::ClockDomain(value)) if matches!(value.kind(), ClockKind::Periodic { .. }))) {
                        diagnostics.push(kernel_error(owner, "record member requires at most one exact periodic ClockDomain"));
                    }
                    if temporal_owner
                        .as_ref()
                        .is_some_and(|expected| expected != &temporal)
                    {
                        diagnostics.push(kernel_error(
                            owner,
                            "record bus members must share one exact temporal ownership",
                        ));
                    } else {
                        temporal_owner = Some(temporal);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{ScalarDomain, ValueType};
    use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Transaction};
    use eqiora_schema::kernel::{
        ClockDomainDef, FieldDef, FieldRole, RationalTime, RecordDef, RecordInstanceDef,
    };

    fn fixture(other_clock: bool) -> (BTreeMap<RawId, KernelNode>, Vec<Edge>, RawId) {
        let ty = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS).unwrap();
        let record = RecordDef::new(
            Id::new(),
            vec![
                ("sensor".into(), ty.clone()),
                ("command".into(), ty.clone()),
            ],
        )
        .unwrap();
        let first = FieldDef::new(Id::new(), ty.clone(), FieldRole::State);
        let second = FieldDef::new(Id::new(), ty, FieldRole::State);
        let instance = RecordInstanceDef::new(
            Id::new(),
            record.id(),
            vec![first.id().erase(), second.id().erase()],
        )
        .unwrap();
        let instance_id = instance.id().erase();
        let clock = ClockDomainDef::periodic(
            Id::new(),
            RationalTime::new(1, 1).unwrap(),
            RationalTime::new(0, 1).unwrap(),
        )
        .unwrap();
        let foreign_clock = ClockDomainDef::periodic(
            Id::new(),
            RationalTime::new(1, 1).unwrap(),
            RationalTime::new(0, 1).unwrap(),
        )
        .unwrap();
        let edges = [
            (first.id().erase(), clock.id().erase()),
            (
                second.id().erase(),
                if other_clock {
                    foreign_clock.id().erase()
                } else {
                    clock.id().erase()
                },
            ),
        ];
        let nodes: BTreeMap<_, _> = [
            record.into(),
            first.into(),
            second.into(),
            instance.into(),
            clock.into(),
            foreign_clock.into(),
        ]
        .into_iter()
        .map(|node: KernelNode| (node.id(), node))
        .collect();
        let mut transaction = Transaction::new("record clocks");
        for node in nodes.values() {
            transaction.push(Op::DefineKernelNode { node: node.clone() });
        }
        for (from, to) in edges {
            transaction.push(Op::Connect {
                from,
                to,
                edge: EdgeKind::ClockedBy,
            });
        }
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let edges = store.snapshot().edges().copied().collect();
        (nodes, edges, instance_id)
    }

    #[test]
    fn homogeneous_bus_rejects_equal_period_foreign_clock() {
        let (nodes, edges, _) = fixture(false);
        let mut errors = Vec::new();
        validate(&nodes, &edges, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        let (nodes, edges, _) = fixture(true);
        validate(&nodes, &edges, &mut errors);
        assert!(
            errors.iter().any(|error| error
                .message()
                .contains("share one exact temporal ownership")),
            "{errors:?}"
        );
    }

    #[test]
    fn record_instance_requires_its_selected_declaration_and_complete_typed_members() {
        let (mut nodes, edges, id) = fixture(false);
        let KernelNode::RecordInstance(instance) = nodes[&id].clone() else {
            panic!("instance")
        };
        let mut errors = Vec::new();
        nodes.remove(&instance.definition().erase());
        validate(&nodes, &edges, &mut errors);
        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("exact selected Record declaration")
        }));
        let (mut nodes, edges, id) = fixture(false);
        let KernelNode::RecordInstance(instance) = nodes[&id].clone() else {
            panic!("instance")
        };
        let member = instance.members()[0];
        nodes.insert(
            member,
            FieldDef::new(
                member.downcast().unwrap(),
                ValueType::boolean(),
                FieldRole::State,
            )
            .into(),
        );
        errors.clear();
        validate(&nodes, &edges, &mut errors);
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("declared type"))
        );
    }
}
