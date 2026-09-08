//! Exhaustive cases use one selector and a linear chain of shared selections.
use super::*;

impl ExpressionLowerer<'_> {
    pub(super) fn lower_case(
        &mut self,
        expression: &LoweringExpression,
        value: &LoweringExpression,
        arms: &[(eqiora_core::ValueLiteral, LoweringExpression)],
    ) -> Result<TypedExpression, Diagnostic> {
        let selector_type = expression_type(self.file, value, self.bindings, None)?;
        crate::enumeration::validate_patterns(
            self.file,
            expression.range(),
            &selector_type.value_type,
            &arms
                .iter()
                .map(|(pattern, _)| pattern.clone())
                .collect::<Vec<_>>(),
        )?;
        let selector = self.lower(value)?;
        let branches = arms
            .iter()
            .map(|(_, value)| self.lower(value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut result = *branches.last().expect("complete nonempty enum cases");
        if arms.len() == 1 {
            let pattern = self
                .builder
                .constant(arms[0].0.clone())
                .map_err(|error| self.builder_error(expression, error))?;
            let condition = self
                .builder
                .compare(
                    eqiora_schema::kernel::ComparisonOp::Equal,
                    selector.id,
                    pattern,
                )
                .map_err(|error| self.builder_error(expression, error))?;
            result.id = self
                .builder
                .select(condition, result.id, result.id)
                .map_err(|error| self.builder_error(expression, error))?;
        }
        // The last arm is a valid fallback only after exact exhaustiveness checking.
        for ((pattern, _), branch) in arms[..arms.len() - 1].iter().zip(&branches).rev() {
            let pattern = self
                .builder
                .constant(pattern.clone())
                .map_err(|error| self.builder_error(expression, error))?;
            let condition = self
                .builder
                .compare(
                    eqiora_schema::kernel::ComparisonOp::Equal,
                    selector.id,
                    pattern,
                )
                .map_err(|error| self.builder_error(expression, error))?;
            result.id = self
                .builder
                .select(condition, branch.id, result.id)
                .map_err(|error| self.builder_error(expression, error))?;
        }
        Ok(result)
    }
}
