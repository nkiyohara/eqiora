//! Exact direct assignments share the expression backend; Newton only sees real residuals.
use super::*;
use eqiora_core::{ScalarDomain, ValueLiteral, ValueType};
use eqiora_schema::kernel::{ExprDag, ExprId};

pub(super) fn supported_type(value: &ValueType) -> bool {
    (value.scalar_domain() == ScalarDomain::Real && value.shape().is_scalar())
        || (value.scalar_domain() == ScalarDomain::Integer && value.array_rank() == 0)
}

pub(super) fn is_discrete(program: &KernelProgram, symbol: SymbolRef) -> bool {
    program
        .execution_symbol_type(symbol)
        .is_some_and(|value| value.scalar_domain() == ScalarDomain::Integer)
}

pub(super) fn is_discrete_id(program: &KernelProgram, id: RawId) -> bool {
    match program.node(id) {
        Some(KernelNode::Field(field)) => is_discrete(program, SymbolRef::Field(field.id())),
        Some(KernelNode::Port(port)) => is_discrete(program, SymbolRef::Port(port.id())),
        _ => false,
    }
}

fn assignment(program: &KernelProgram, dag: &ExprDag, root: ExprId) -> Option<(SymbolRef, ExprId)> {
    let ExprNode::Sub(a, b) = dag.nodes().get(root.index() as usize)? else {
        return None;
    };
    for (target, rhs) in [(*a, *b), (*b, *a)] {
        if let Some(ExprNode::Symbol(symbol)) = dag.nodes().get(target.index() as usize)
            && matches!(
                symbol,
                SymbolRef::Field(_) | SymbolRef::Pre(_) | SymbolRef::Next(_) | SymbolRef::Port(_)
            )
            && is_discrete(program, *symbol)
        {
            return Some((*symbol, rhs));
        }
    }
    None
}

pub(super) fn numerical_roots(program: &KernelProgram, dag: &ExprDag) -> Vec<ExprId> {
    dag.roots()
        .iter()
        .copied()
        .filter(|root| assignment(program, dag, *root).is_none())
        .collect()
}

pub(super) fn typed_fields(
    program: &KernelProgram,
    state: &RuntimeState,
) -> Result<BTreeMap<RawId, ValueLiteral>, Vec<Diagnostic>> {
    let mut fields = state.discrete_fields.clone();
    for (&id, &value) in &state.fields {
        let Some(KernelNode::Field(field)) = program.node(id) else {
            continue;
        };
        fields.insert(
            id,
            ValueLiteral::from_real(field.value_type().clone(), value)
                .map_err(|_| vec![execution_error("accepted Field value is not finite", 0.0)])?,
        );
    }
    Ok(fields)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stage(
    program: &KernelProgram,
    plan: &ExecutionPlan,
    state: &mut RuntimeState,
    relations: &BTreeSet<RawId>,
    time: f64,
    initial: bool,
    backend: &impl ExpressionBackend,
) -> Result<(), Diagnostic> {
    let mut pending = Vec::new();
    let mut targets = std::collections::HashSet::new();
    for &owner in relations {
        let Some(KernelNode::Relation(relation)) = program.node(owner) else {
            continue;
        };
        for &root in relation.residuals().roots() {
            if let Some((target, rhs)) = assignment(program, relation.residuals(), root) {
                let target = if initial {
                    match target {
                        SymbolRef::Pre(id) => SymbolRef::Field(id),
                        other => other,
                    }
                } else {
                    target
                };
                let allowed = match target {
                    SymbolRef::Next(_) => !initial,
                    SymbolRef::Field(id) => initial || is_clocked_variable(program, id.erase()),
                    SymbolRef::Port(id) => {
                        !initial
                            && is_output_port(program, id.erase())
                            && !plan.signal_sources.contains_key(&id.erase())
                    }
                    _ => false,
                };
                if !allowed || !targets.insert(target) {
                    return Err(execution_error(
                        "exact discrete assignments require one direct target at its owning activation",
                        time,
                    ));
                }
                pending.push((owner, relation.residuals(), target, rhs));
            }
        }
    }
    while !pending.is_empty() {
        let before = pending.len();
        let mut waiting = Vec::new();
        for (owner, dag, target, rhs) in pending {
            let empty = BTreeMap::new();
            let context = EvalContext {
                program,
                time,
                fields: &state.fields,
                field_candidates: &empty,
                derivatives: &state.derivatives,
                next_fields: &empty,
                ports: &state.ports,
                port_candidates: &empty,
                signal_sources: &plan.signal_sources,
                physical: &state.physical,
                physical_candidates: &BTreeMap::new(),
                discrete_fields: &state.discrete_fields,
                discrete_ports: &state.discrete_ports,
                discrete_next: &state.discrete_next,
            };
            let mut missing = false;
            let result = backend.evaluate(owner, dag, &[rhs], &mut |symbol| {
                let value = if initial
                    && !matches!(symbol, SymbolRef::Parameter(_))
                    && !is_discrete(program, symbol)
                {
                    None
                } else {
                    evaluate::resolve_symbol(symbol, &context)
                };
                missing |= value.is_none();
                value
            });
            if missing {
                waiting.push((owner, dag, target, rhs));
                continue;
            }
            let mut result = result?;
            let value = result
                .pop()
                .ok_or_else(|| execution_error("exact assignment has no value", time))?;
            if Some(value.value_type().clone()) != program.execution_symbol_type(target) {
                return Err(execution_error(
                    "exact assignment value differs from its complete target type",
                    time,
                ));
            }
            match target {
                SymbolRef::Next(id) => {
                    state.discrete_next.insert(id.erase(), value);
                }
                SymbolRef::Field(id) => {
                    state.discrete_fields.insert(id.erase(), value);
                }
                SymbolRef::Port(id) => {
                    state.discrete_ports.insert(id.erase(), value);
                }
                _ => unreachable!(),
            }
        }
        if waiting.len() == before {
            return Err(execution_error(
                "exact discrete assignments are cyclic or lack an accepted input",
                time,
            ));
        }
        pending = waiting;
    }
    if initial
        && plan.fields.iter().any(|id| {
            is_discrete_id(program, *id)
                && !is_clocked_variable(program, *id)
                && !state.discrete_fields.contains_key(id)
        })
    {
        return Err(execution_error(
            "exact discrete State requires an explicit initial assignment",
            time,
        ));
    }
    Ok(())
}
