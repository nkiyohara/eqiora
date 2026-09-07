use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_schema::kernel::{ExprNode, KernelNode, SymbolRef};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};
use eqiora_time::ImplicitDaeInitialization;

use crate::time::invalid_time;

pub(crate) fn initialize(
    kernel: &KernelProgram,
    fields: &[Id<kinds::Field>],
    relation: Id<kinds::Relation>,
    config: ReferenceConfig,
) -> Result<ImplicitDaeInitialization, Diagnostic> {
    let initial = Interpreter::new()
        .initialize(kernel, config)
        .map_err(|diagnostics| {
            diagnostics.into_iter().next().unwrap_or_else(|| {
                invalid_time(relation, "fresh initialization failed without a diagnostic")
            })
        })?;
    let state = fields
        .iter()
        .map(|field| {
            initial
                .fields()
                .get(&field.erase())
                .copied()
                .ok_or_else(|| {
                    invalid_time(
                        relation,
                        "fresh initialization omitted a required state coordinate",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let derivative = fields.iter().map(|field| {
        if let Some(value) = initial.derivatives().get(&field.erase()) {
            return Ok(*value);
        }
        // Algebraic coordinates have no derivative unknown in this index-one
        // partition. Zero is the residual-adapter convention for that absent
        // coordinate, not an inferred initial condition or physical derivative.
        let has_derivative = kernel.nodes().any(|node| match node {
            KernelNode::Relation(definition) => definition.residuals().nodes().iter().any(|node| {
                matches!(node, ExprNode::Symbol(SymbolRef::Derivative(candidate)) if candidate == field)
            }),
            _ => false,
        });
        if has_derivative {
            Err(invalid_time(relation, "fresh initialization omitted a differential coordinate"))
        } else {
            Ok(0.0)
        }
    }).collect::<Result<Vec<_>, _>>()?;
    ImplicitDaeInitialization::accepted(state, derivative)
}
