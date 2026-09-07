//! Static admission and dependency classification for reference execution.

use super::*;

impl ExecutionPlan {
    pub(super) fn new(program: &KernelProgram) -> Result<Self, Diagnostic> {
        for node in program.nodes() {
            if let KernelNode::Parameter(parameter) = node
                && (parameter.value_type().scalar_domain() != eqiora_core::ScalarDomain::Real
                    || !parameter.value_type().shape().is_scalar())
            {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "reference execution requires real scalar Parameters",
                ));
            }
            if let KernelNode::Relation(relation) = node
                && relation.residuals().nodes().iter().any(|node| {
                    matches!(
                        node,
                        ExprNode::Array { .. } | ExprNode::Index { .. } | ExprNode::Complex { .. }
                    )
                })
            {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "reference execution does not admit array, index, or complex construction",
                ));
            }
            if let KernelNode::Domain(domain) = node
                && let DomainKind::ScalarPhysical {
                    across_type,
                    through_type,
                } = domain.kind()
                && (across_type.scalar_domain() != eqiora_core::ScalarDomain::Real
                    || through_type.scalar_domain() != eqiora_core::ScalarDomain::Real)
            {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "reference execution requires real scalar physical quantities",
                ));
            }
            if let KernelNode::Field(field) = node
                && (field.value_type().scalar_domain() != eqiora_core::ScalarDomain::Real
                    || !field.shape().is_scalar())
            {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "reference execution requires real scalar Fields",
                ));
            }
            if let KernelNode::Field(field) = node
                && program.edges().iter().any(|edge| {
                    edge.from() == field.id().erase()
                        && edge.kind() == eqiora_graph::EdgeKind::DefinedOn
                        && edge.to().kind() == eqiora_core::EntityKind::Domain
                })
            {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "reference execution does not realize distributed Fields",
                ));
            }
            if let KernelNode::Port(port) = node
                && let Some((_, value_type)) = port.signal_contract()
                && (value_type.scalar_domain() != eqiora_core::ScalarDomain::Real
                    || !value_type.shape().is_scalar())
            {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "reference execution requires real scalar signal Ports",
                ));
            }
        }
        let signal_sources = signal_sources(program)?;
        let physical_systems = physical_systems(program)?;
        let physical_unknowns = physical_systems
            .iter()
            .flat_map(|system| system.unknowns().iter().copied())
            .collect();
        let mut continuous_relations = BTreeSet::new();
        let mut periodic = Vec::new();
        let mut periodic_clocks = BTreeSet::new();
        let mut events = Vec::new();

        for node in program.nodes() {
            let KernelNode::Activation(activation) = node else {
                continue;
            };
            let activation_id = activation.id().erase();
            let relations = edge_targets(program, activation_id, eqiora_graph::EdgeKind::Activates);
            match activation.kind() {
                ActivationKind::Continuous => continuous_relations.extend(relations),
                ActivationKind::Periodic => {
                    let Some(clock_id) =
                        edge_targets(program, activation_id, eqiora_graph::EdgeKind::ClockedBy)
                            .into_iter()
                            .next()
                    else {
                        return Err(execution_error(
                            "periodic Activation has no validated ClockDomain",
                            0.0,
                        ));
                    };
                    let Some(KernelNode::ClockDomain(clock)) = program.node(clock_id) else {
                        return Err(execution_error(
                            "validated periodic ClockDomain definition is unavailable",
                            0.0,
                        ));
                    };
                    let ClockKind::Periodic { period, phase } = clock.kind() else {
                        return Err(execution_error(
                            "validated periodic clock changed kind",
                            0.0,
                        ));
                    };
                    periodic_clocks.insert(clock_id);
                    periodic.push(PeriodicTask {
                        relations,
                        period,
                        next: phase,
                    });
                }
                ActivationKind::Event { guard, direction } => {
                    events.push(EventTask {
                        activation: activation_id,
                        relations,
                        guard: guard.clone(),
                        direction: *direction,
                    });
                }
                ActivationKind::Guard { .. } => {
                    return Err(Diagnostic::error(
                        codes::NOT_IMPLEMENTED,
                        "guard activation follows the event foundation milestone",
                    )
                    .with_graph_path(kernel_path(activation_id)));
                }
                _ => {
                    return Err(Diagnostic::error(
                        codes::NOT_IMPLEMENTED,
                        "Activation kind is newer than this reference interpreter",
                    )
                    .with_graph_path(kernel_path(activation_id)));
                }
            }
        }

        events.sort_by_key(|event| event.activation);
        if !physical_systems.is_empty() && !events.is_empty() {
            return Err(Diagnostic::error(
                codes::NOT_IMPLEMENTED,
                "joint scalar physical execution does not yet compose zero-crossing events",
            )
            .with_graph_path(kernel_path(events[0].activation)));
        }
        if !physical_systems.is_empty()
            && let Some(second_clock) = periodic_clocks.iter().nth(1).copied()
        {
            return Err(Diagnostic::error(
                codes::NOT_IMPLEMENTED,
                "joint scalar physical execution admits at most one periodic ClockDomain",
            )
            .with_graph_path(kernel_path(second_clock)));
        }

        let mut discrete_fields = BTreeSet::new();
        let mut discrete_ports = BTreeSet::new();
        for relation in periodic
            .iter()
            .flat_map(|task| &task.relations)
            .chain(events.iter().flat_map(|task| &task.relations))
        {
            for symbol in relation_symbols(program, *relation)? {
                match symbol {
                    SymbolRef::Next(field) => {
                        discrete_fields.insert(field.erase());
                    }
                    SymbolRef::Port(port) if is_output_port(program, port.erase()) => {
                        discrete_ports.insert(port.erase());
                    }
                    _ => {}
                }
            }
        }

        let mut differential_fields = BTreeSet::new();
        let mut continuous_field_references = BTreeSet::new();
        let mut continuous_ports = BTreeSet::new();
        for &relation in &continuous_relations {
            for symbol in relation_symbols(program, relation)? {
                match symbol {
                    SymbolRef::Derivative(field) => {
                        differential_fields.insert(field.erase());
                    }
                    SymbolRef::Field(field) => {
                        continuous_field_references.insert(field.erase());
                    }
                    SymbolRef::Port(port) => {
                        let source = signal_sources
                            .get(&port.erase())
                            .copied()
                            .unwrap_or_else(|| port.erase());
                        if is_output_port(program, source) && !discrete_ports.contains(&source) {
                            continuous_ports.insert(source);
                        }
                    }
                    SymbolRef::Pre(_) | SymbolRef::Next(_) => {
                        return Err(Diagnostic::error(
                            codes::INVALID_KERNEL_DEFINITION,
                            "continuous Relations cannot read Pre or Next symbols",
                        )
                        .with_graph_path(kernel_path(relation)));
                    }
                    _ => {}
                }
            }
        }
        let algebraic_fields = continuous_field_references
            .difference(&differential_fields)
            .copied()
            .filter(|field| !discrete_fields.contains(field))
            .collect();
        let fields = program
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Field(field) => Some(field.id().erase()),
                _ => None,
            })
            .collect();

        let initial_relations = program
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Relation(relation) if relation.is_initial() => {
                    Some(relation.id().erase())
                }
                _ => None,
            })
            .collect();
        Ok(Self {
            initial_relations,
            continuous_relations,
            periodic,
            events,
            differential_fields,
            algebraic_fields,
            continuous_ports,
            signal_sources,
            physical_systems,
            physical_unknowns,
            fields,
        })
    }
}
