//! Static predicate values retain both operand types and expression dependencies.
use super::*;
use eqiora_schema::kernel::typing::ExpressionType;

pub(super) fn negate(
    file: &str,
    range: TextRange,
    operand: EvaluatedParameter,
) -> Result<EvaluatedParameter, Diagnostic> {
    ExpressionType::<()>::new(operand.value_type.value_type().clone(), None)
        .logical_not()
        .map_err(|error| {
            source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
        })?;
    Ok(EvaluatedParameter {
        value: operand
            .value
            .as_ref()
            .and_then(ValueLiteral::as_bool)
            .map(|value| ValueLiteral::boolean(!value)),
        value_type: EvaluatedType::Known(ValueType::boolean()),
        bare_literal: false,
        expression: operand
            .expression
            .map(|value| LoweringExpression::logical_not(value, range)),
        lineage: transform_lineage(operand.lineage),
    })
}

pub(super) fn combine(
    file: &str,
    range: TextRange,
    operator: BinaryOp,
    left: EvaluatedParameter,
    right: EvaluatedParameter,
) -> Result<EvaluatedParameter, Diagnostic> {
    let error = |message: String| source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message);
    let comparison = crate::lower::comparison_operator(operator);
    let project = |value: &EvaluatedParameter, peer: &EvaluatedParameter| {
        ExpressionType::<()>::new(
            value.value_type.value_type().clone().with_dimension(
                value
                    .value_type
                    .dimension()
                    .or(peer.value_type.dimension())
                    .unwrap_or(DimExponents::DIMENSIONLESS),
            ),
            None,
        )
    };
    let lhs = project(&left, &right);
    let rhs = project(&right, &left);
    if let Some(op) = comparison {
        lhs.compare(op, rhs)
    } else {
        lhs.and(rhs)
    }
    .map_err(|violation| error(violation.to_string()))?;
    let value = if comparison.is_some() {
        left.value
            .as_ref()
            .zip(right.value.as_ref())
            .map(|(left, right)| {
                let result = match operator {
                    BinaryOp::Equal => left.checked_equal(right),
                    BinaryOp::NotEqual => left.checked_equal(right).map(|value| !value),
                    BinaryOp::Less => left.checked_order(right).map(|order| order.is_lt()),
                    BinaryOp::LessEqual => left.checked_order(right).map(|order| order.is_le()),
                    BinaryOp::Greater => left.checked_order(right).map(|order| order.is_gt()),
                    BinaryOp::GreaterEqual => left.checked_order(right).map(|order| order.is_ge()),
                    _ => unreachable!("comparison mapping checked"),
                };
                result
                    .map(ValueLiteral::boolean)
                    .map_err(|violation| error(violation.to_string()))
            })
            .transpose()?
    } else {
        left.value
            .as_ref()
            .and_then(ValueLiteral::as_bool)
            .and_then(|lhs| {
                if operator == BinaryOp::And && !lhs || operator == BinaryOp::Or && lhs {
                    Some(lhs)
                } else {
                    right.value.as_ref().and_then(ValueLiteral::as_bool)
                }
            })
            .map(ValueLiteral::boolean)
    };
    Ok(EvaluatedParameter {
        value,
        value_type: EvaluatedType::Known(ValueType::boolean()),
        bare_literal: false,
        lineage: combine_lineages(left.lineage, right.lineage),
        expression: left
            .expression
            .zip(right.expression)
            .map(|(left, right)| LoweringExpression::binary(operator, left, right, range)),
    })
}

pub(super) fn untyped_numeric_tree(expression: &Expr) -> bool {
    match expression.kind() {
        ExprKind::Number(_) => true,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => untyped_numeric_tree(value),
        ExprKind::Binary {
            op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow,
            left,
            right,
        } => untyped_numeric_tree(left) && untyped_numeric_tree(right),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evaluate(source: &str) -> Result<EvaluatedParameter, Diagnostic> {
        let text = format!("model M() {{ let result = {source}; }}");
        let document = eqiora_lang::parse("predicate.eqi", &text)
            .into_document()
            .unwrap();
        let eqiora_lang::Item::Let(declaration) = &document.models()[0].items()[0] else {
            panic!("let");
        };
        expression_eval::evaluate_parameter_expression(
            "predicate.eqi",
            declaration.value(),
            ExpressionContext::Let,
            &mut |name, range| {
                if name != "n" {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        "predicate.eqi",
                        range,
                        "unknown Parameter",
                    ));
                }
                let value = ValueLiteral::from_integer(
                    ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS),
                    9_007_199_254_740_993,
                )
                .unwrap();
                Ok(SymbolicParameterValue {
                    value: Some(value.clone()),
                    value_type: value.value_type().clone(),
                    expression: Some(LoweringExpression::literal(value, range)),
                    lineage: Some(ParameterLineage::Derived),
                })
            },
            &mut |_| None,
        )
    }
    #[test]
    fn static_predicates_contextualize_exact_operands_not_boolean_results() {
        for (source, expected) in [
            ("n == 9007199254740993", true),
            ("9007199254740992 < n", true),
            ("n == 9007199254740992 + 1", true),
            ("n == 9007199254740992", false),
            ("not (n < -9223372036854775808)", true),
            ("1/2 == 0.5", true),
        ] {
            assert_eq!(
                evaluate(source).unwrap().value.unwrap().as_bool(),
                Some(expected),
                "{source}"
            );
        }
        assert!(evaluate("n == to_real(n)").is_err());
        assert!(evaluate("n == (9223372036854775807 + 1) - 1").is_err());
    }
    #[test]
    fn short_circuit_suppresses_values_but_not_types_names_or_lineage() {
        for (source, expected) in [
            ("false and (math.sqrt(-1) > 0)", false),
            ("true or (quotient(1,0) > 0)", true),
            ("false and ((9223372036854775807 + 1) > n)", false),
        ] {
            let value = evaluate(source).unwrap();
            assert_eq!(value.value.unwrap().as_bool(), Some(expected));
            assert!(value.expression.is_some());
        }
        let retained = evaluate("false and (n == 0)").unwrap();
        assert_eq!(retained.lineage, Some(ParameterLineage::Derived));
        for source in [
            "false and missing",
            "true or 1",
            "false and (1[m] < 1[s])",
            "true and (math.sqrt(-1) > 0)",
            "false or (quotient(1,0) > 0)",
        ] {
            assert!(evaluate(source).is_err(), "{source}");
        }
    }
}
