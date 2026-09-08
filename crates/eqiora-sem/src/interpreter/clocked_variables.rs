//! Algebraic tick values have presence at one accepted activation, never history.
use super::*;

pub(super) fn is_clocked_variable(program: &KernelProgram, field: RawId) -> bool {
    matches!(program.node(field), Some(KernelNode::Field(definition)) if definition.role() == eqiora_schema::kernel::FieldRole::Variable)
        && program
            .edges()
            .iter()
            .any(|edge| edge.from() == field && edge.kind() == eqiora_graph::EdgeKind::ClockedBy)
}

pub(super) fn clear_clocked_variables(program: &KernelProgram, state: &mut RuntimeState) {
    state
        .discrete_fields
        .retain(|field, _| !is_clocked_variable(program, *field));
    state
        .fields
        .retain(|field, _| !is_clocked_variable(program, *field));
}

/// Fresh algebraic tick values use the configured guess and require a unique solve.
pub(super) fn solve_seed(
    program: &KernelProgram,
    variables: &[Variable],
    state: &RuntimeState,
    config: ReferenceConfig,
) -> (Vec<f64>, bool) {
    let mut check_rank = false;
    let initial = variables
        .iter()
        .map(|variable| match variable {
            Variable::Field(field) if is_clocked_variable(program, *field) => {
                check_rank = true;
                config.initial_guess()
            }
            _ => variable_value(*variable, state),
        })
        .collect();
    (initial, check_rank)
}
