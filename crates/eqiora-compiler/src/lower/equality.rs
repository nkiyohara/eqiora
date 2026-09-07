//! The one checked authored-equality boundary, shared by body checking and lowering.

use eqiora_lang::{Expr, ExprKind, UnaryOp};
use eqiora_schema::kernel::typing::{self, ExpressionType, TypeViolation};

/// Parentheses are already represented by expression ranges. Only a literal
/// zero, optionally beneath unary negation, is contextual; named or computed
/// values never gain a contextual type, even when a later substitution is zero.
pub(crate) fn is_contextual_zero(mut expression: &Expr) -> bool {
    loop {
        match expression.kind() {
            ExprKind::Number(value) => return *value == 0.0,
            ExprKind::Unary {
                op: UnaryOp::Neg,
                value,
            } => expression = value,
            _ => return false,
        }
    }
}

pub(crate) fn is_literal_zero(mut expression: &Expr) -> bool {
    loop {
        match expression.kind() {
            ExprKind::Number(value) => return *value == 0.0,
            ExprKind::Quantity { value, .. } => return value.is_zero(),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                value,
            } => expression = value,
            _ => return false,
        }
    }
}

pub(crate) struct CheckedEquality<I> {
    pub(crate) left: ExpressionType<I>,
    pub(crate) right: ExpressionType<I>,
    pub(crate) residual: ExpressionType<I>,
}

/// Both operands must already have passed their ordinary expression checks.
/// Contextual literal zero inherits a mathematical type, never a new spatial
/// support. Explicit typed literals retain every part of their declared type.
pub(crate) fn check<I: Clone + Eq>(
    mut left: ExpressionType<I>,
    mut right: ExpressionType<I>,
    contextual_left_zero: bool,
    contextual_right_zero: bool,
) -> Result<CheckedEquality<I>, TypeViolation<I>> {
    if contextual_left_zero && !contextual_right_zero {
        left = ExpressionType::new(right.value_type.clone(), None);
    }
    if contextual_right_zero && !contextual_left_zero {
        right = ExpressionType::new(left.value_type.clone(), None);
    }
    let residual = typing::additive(&left, &right)?;
    Ok(CheckedEquality {
        left,
        right,
        residual,
    })
}

#[cfg(test)]
mod tests;
