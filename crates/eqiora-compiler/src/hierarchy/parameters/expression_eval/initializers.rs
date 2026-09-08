//! Target-directed initializer typing with explicit value demand.
use super::*;

pub(in crate::hierarchy::parameters) fn evaluate_initializer(
    file: &str,
    expression: &Expr,
    context: ExpressionContext<'_>,
    resolve: &mut impl FnMut(&str, TextRange) -> Result<SymbolicParameterValue, Diagnostic>,
    target: ValueType,
    label: &str,
    (resolve_clock, resolve_frame): StaticContexts<'_>,
) -> Result<EvaluatedParameter, Diagnostic> {
    evaluate_initializer_mode(
        file,
        expression,
        context,
        resolve,
        target,
        (label, true),
        (resolve_clock, resolve_frame),
    )
}

pub(in crate::hierarchy::parameters) fn evaluate_initializer_mode(
    file: &str,
    expression: &Expr,
    context: ExpressionContext<'_>,
    resolve: &mut impl FnMut(&str, TextRange) -> Result<SymbolicParameterValue, Diagnostic>,
    target: ValueType,
    (label, evaluate_values): (&str, bool),
    (resolve_clock, resolve_frame): StaticContexts<'_>,
) -> Result<EvaluatedParameter, Diagnostic> {
    let mut evaluated = if matches!(expression.kind(), ExprKind::Call { callee, .. } if callee.as_str() == "tensor_value")
    {
        super::super::tensor_values::evaluate(
            file,
            expression,
            context,
            resolve,
            (&mut *resolve_clock, &mut *resolve_frame),
            Some(&target),
            evaluate_values,
        )?
    } else if matches!(expression.kind(), ExprKind::Array(_))
        || matches!(expression.kind(), ExprKind::Call { callee, .. } if callee.as_str() == "math.complex")
    {
        super::super::value_expressions::evaluate_mode(
            file,
            expression,
            context,
            resolve,
            (&mut *resolve_clock, &mut *resolve_frame),
            Some(&target),
            evaluate_values,
        )?
    } else {
        evaluate_mode(
            file,
            expression,
            context,
            resolve,
            (&mut *resolve_clock, &mut *resolve_frame),
            (target.scalar_domain() == ScalarDomain::Integer).then_some(ScalarDomain::Integer),
            evaluate_values,
        )?
    };
    if !evaluate_values && evaluated.bare_literal {
        // Literal projection supplies its exact authored value for contextual typing;
        // no arithmetic subtree is evaluated on this path.
        evaluated = evaluate_mode(
            file,
            expression,
            context,
            resolve,
            (&mut *resolve_clock, &mut *resolve_frame),
            (target.scalar_domain() == ScalarDomain::Integer).then_some(ScalarDomain::Integer),
            true,
        )?;
    }
    let mut result: EvaluatedParameter =
        coerce_parameter_with_label(file, expression.range(), evaluated, target, label, true)?
            .into();
    if !evaluate_values {
        result.value = None;
    }
    Ok(result)
}
