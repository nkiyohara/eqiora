//! Intrinsic finite-fold typing uses the same capture-aware ordinal projection.
use super::*;

impl ExpressionChecker<'_, '_, '_> {
    pub(super) fn reduction(
        &mut self,
        expression: &Expr,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let ExprKind::Reduction {
            operation,
            binder,
            value,
        } = expression.kind()
        else {
            unreachable!()
        };
        let invalid = |message: &str| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                message,
            )
        };
        if self.scope.symbols.contains_key(binder.member())
            || self.scope.static_values.contains_key(binder.member())
            || self.scope.children.contains_key(binder.member())
            || self.scope.index_sets.contains_key(binder.member())
        {
            return Err(invalid(
                "reduction binder collides with an existing declaration",
            ));
        }
        crate::hierarchy::reductions::preflight(
            self.scope.file,
            expression,
            &mut |name| self.scope.index_sets.get(name).copied().flatten(),
            self.scope.elaborator.limits.max_parameter_terms,
        )?;
        let extent = self
            .scope
            .index_sets
            .get(binder.set().as_str())
            .copied()
            .flatten()
            .ok_or_else(|| invalid("reduction requires an exact resolved IndexSet extent"))?;
        let mut result = None;
        for ordinal in 0..extent {
            let term =
                crate::hierarchy::reductions::instantiate(self.scope.file, value, binder, ordinal)?;
            let term = self.check(&term)?;
            if !term.value_type.shape().is_scalar()
                || term.value_type.index_set().is_some()
                || !matches!(
                    term.value_type.scalar_domain(),
                    eqiora_core::ScalarDomain::Real
                        | eqiora_core::ScalarDomain::Complex
                        | eqiora_core::ScalarDomain::Integer
                )
            {
                return Err(invalid(
                    "finite reductions require ordinary real, complex, or integer scalar operands",
                ));
            }
            if matches!(
                operation,
                eqiora_lang::ReductionOp::Min | eqiora_lang::ReductionOp::Max
            ) {
                term.clone()
                    .ordered_selection(term.clone())
                    .map_err(|error| type_error(self.scope.file, expression, error))?;
            }
            result = Some(match result {
                None => term,
                Some(previous) => match operation {
                    eqiora_lang::ReductionOp::Sum => ExpressionType::sum(previous, term),
                    eqiora_lang::ReductionOp::Product => typing::multiply(&previous, &term),
                    eqiora_lang::ReductionOp::Min | eqiora_lang::ReductionOp::Max => {
                        previous.ordered_selection(term)
                    }
                }
                .map_err(|error| type_error(self.scope.file, expression, error))?,
            });
        }
        result.ok_or_else(|| invalid("finite reduction requires a nonempty IndexSet"))
    }
}
