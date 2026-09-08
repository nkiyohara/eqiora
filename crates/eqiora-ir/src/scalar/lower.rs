//! Scalar value projection from validated semantic expression operands.

use super::*;
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode};

impl ScalarOperatorIr {
    /// Lower a canonical expression DAG into dense symbol slots and scalar
    /// instructions without changing operation order. Sample/Hold reuse their
    /// operand value; activation and memory admission remain with the validated
    /// KernelProgram retained by the executor, not this scalar value projection.
    ///
    /// # Errors
    /// Returns `EQ0701` if an operand/root index is inconsistent with the DAG
    /// contract.
    pub fn lower(expression: &ExprDag) -> Result<Self, Diagnostic> {
        let mut typed_constants = Vec::new();
        let mut symbols = Vec::new();
        let mut symbol_slots = HashMap::new();
        let mut instructions = Vec::with_capacity(expression.nodes().len());
        let mut values = Vec::with_capacity(expression.nodes().len());
        for (index, node) in expression.nodes().iter().enumerate() {
            if let ExprNode::Sample { value, .. } | ExprNode::Hold(value) = node {
                values.push(value_id(*value, &values)?);
                continue;
            }
            let instruction = match node {
                ExprNode::Constant(value) => {
                    if let Some(real) = value.real_scalar_value() {
                        Instruction::Constant(real)
                    } else if value.value_type().scalar_domain()
                        == eqiora_core::ScalarDomain::Integer
                    {
                        let slot =
                            u32::try_from(typed_constants.len()).map_err(|_| ir_size_error())?;
                        typed_constants.push(value.clone());
                        Instruction::TypedConstant(slot)
                    } else {
                        return Err(ir_builder_error(
                            "scalar IR requires real scalar or exact discrete constants",
                        ));
                    }
                }
                ExprNode::Quotient(a, b) => {
                    Instruction::Quotient(value_id(*a, &values)?, value_id(*b, &values)?)
                }
                ExprNode::Remainder(a, b) => {
                    Instruction::Remainder(value_id(*a, &values)?, value_id(*b, &values)?)
                }
                ExprNode::ToReal(value) => Instruction::ToReal(value_id(*value, &values)?),
                ExprNode::Ordinal(value) => Instruction::Ordinal(value_id(*value, &values)?),
                ExprNode::ToInteger(value) => Instruction::ToInteger(value_id(*value, &values)?),
                ExprNode::Symbol(symbol) => {
                    let next_slot = u32::try_from(symbols.len()).map_err(|_| ir_size_error())?;
                    let slot = *symbol_slots.entry(*symbol).or_insert_with(|| {
                        symbols.push(*symbol);
                        SymbolSlot(next_slot)
                    });
                    Instruction::Read(slot)
                }
                ExprNode::Neg(value) => Instruction::Neg(value_id(*value, &values)?),
                ExprNode::Add(left, right) => {
                    Instruction::Add(value_id(*left, &values)?, value_id(*right, &values)?)
                }
                ExprNode::Sub(left, right) => {
                    Instruction::Sub(value_id(*left, &values)?, value_id(*right, &values)?)
                }
                ExprNode::Mul(left, right) => {
                    Instruction::Mul(value_id(*left, &values)?, value_id(*right, &values)?)
                }
                ExprNode::Div(left, right) => {
                    Instruction::Div(value_id(*left, &values)?, value_id(*right, &values)?)
                }
                ExprNode::PowI(base, exponent) => {
                    Instruction::PowI(value_id(*base, &values)?, *exponent)
                }
                _ => {
                    return Err(Diagnostic::error(
                        codes::INVALID_OPERATOR_IR,
                        "expression node is newer than scalar Operator IR",
                    )
                    .with_graph_path(ir_path(index)));
                }
            };
            values.push(ValueId(
                u32::try_from(instructions.len()).map_err(|_| ir_size_error())?,
            ));
            instructions.push(instruction);
        }
        let roots = expression
            .roots()
            .iter()
            .map(|root| value_id(*root, &values))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            source_values: values,
            typed_constants,
            symbols,
            instructions,
            roots,
        })
    }
}

fn value_id(id: ExprId, values: &[ValueId]) -> Result<ValueId, Diagnostic> {
    usize::try_from(id.index())
        .ok()
        .and_then(|index| values.get(index))
        .copied()
        .ok_or_else(|| invalid_value(id, values.len()))
}

fn invalid_value(id: ExprId, upper_bound: usize) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_OPERATOR_IR,
        format!(
            "expression value {} is not below the instruction bound {upper_bound}",
            id.index()
        ),
    )
    .with_graph_path(ir_path(upper_bound))
}
