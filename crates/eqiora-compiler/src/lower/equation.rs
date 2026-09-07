//! Ordered equations retained through hierarchy substitution.

use super::{LoweringExpression, TextRange, equality};

/// Ordered equality, retained through hierarchy substitution until typed lowering.
#[derive(Debug, Clone)]
pub(crate) struct LoweringEquation {
    pub(crate) left: LoweringExpression,
    pub(crate) right: LoweringExpression,
    pub(crate) contextual_left_zero: bool,
    pub(crate) contextual_right_zero: bool,
    pub(crate) literal_right_zero: bool,
    pub(crate) range: TextRange,
}

impl LoweringEquation {
    pub(crate) fn from_source(equation: &eqiora_lang::Equation) -> Self {
        Self::rewritten(
            equation,
            LoweringExpression::from_source(equation.left()),
            LoweringExpression::from_source(equation.right()),
        )
    }

    pub(crate) fn rewritten(
        equation: &eqiora_lang::Equation,
        left: LoweringExpression,
        right: LoweringExpression,
    ) -> Self {
        Self {
            left,
            right,
            contextual_left_zero: equality::is_contextual_zero(equation.left()),
            contextual_right_zero: equality::is_contextual_zero(equation.right()),
            literal_right_zero: equality::is_literal_zero(equation.right()),
            range: equation.range(),
        }
    }
}
