//! Finite reductions retain ordinary operand DAGs in exact index order.
use super::*;

pub(super) fn rewrite(
    file: &str,
    expression: &Expr,
    scope: &Scope,
    active: Option<ActiveBoundaryMember<'_>>,
) -> Result<LoweringExpression, Diagnostic> {
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
            file,
            expression.range(),
            message,
        )
    };
    if scope.symbols.contains_key(binder.member())
        || scope.values.contains_key(binder.member())
        || scope.children.contains_key(binder.member())
        || scope.index_sets.contains_key(binder.member())
    {
        return Err(invalid(
            "reduction binder collides with an existing declaration",
        ));
    }
    super::super::reductions::preflight(
        file,
        expression,
        &mut |name| scope.index_set(name).map(|set| set.extent()),
        &scope.symbolic_parameters(),
        scope.reduction_terms_limit,
    )?;
    let set = scope
        .index_set(binder.set().as_str())
        .ok_or_else(|| invalid("reduction requires an exact resolved IndexSet"))?;
    let mut result = None;
    for ordinal in 0..set.extent() {
        let member = scope.with_index_member(binder.member(), set, ordinal)?;
        let term = rewrite_expression_with_boundary_member(file, value, &member, active)?;
        result = Some(match result {
            None => term,
            Some(previous) => match operation {
                eqiora_lang::ReductionOp::Sum => LoweringExpression::binary(
                    eqiora_lang::BinaryOp::Add,
                    previous,
                    term,
                    expression.range(),
                ),
                eqiora_lang::ReductionOp::Product => LoweringExpression::binary(
                    eqiora_lang::BinaryOp::Mul,
                    previous,
                    term,
                    expression.range(),
                ),
                eqiora_lang::ReductionOp::Min | eqiora_lang::ReductionOp::Max => {
                    LoweringExpression::extremum(
                        matches!(operation, eqiora_lang::ReductionOp::Min),
                        previous,
                        term,
                        expression.range(),
                    )
                }
            },
        });
    }
    result.ok_or_else(|| invalid("finite reduction requires a nonempty IndexSet"))
}
