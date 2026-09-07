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
        .fields
        .retain(|field, _| !is_clocked_variable(program, *field));
}
