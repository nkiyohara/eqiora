use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum ExpressionContext {
    Binding,
    Default,
    Let,
}

impl ExpressionContext {
    pub(super) fn qualified_name_message(self, path: &impl std::fmt::Display) -> String {
        match self {
            Self::Binding => format!(
                "qualified name `{path}` is not allowed in a compile-time Parameter binding"
            ),
            Self::Default => {
                format!("qualified name `{path}` is not allowed in a Parameter default")
            }
            Self::Let => format!("qualified name `{path}` is not allowed in a let alias"),
        }
    }

    pub(super) fn call_message(self, callee: &str) -> String {
        match self {
            Self::Binding => {
                format!("operator `{callee}(...)` is not allowed in a compile-time binding")
            }
            Self::Default => {
                format!("operator `{callee}(...)` is not allowed in a Parameter default")
            }
            Self::Let => format!("operator `{callee}(...)` is not allowed in a let alias"),
        }
    }

    pub(super) const fn unsupported_message(self) -> &'static str {
        match self {
            Self::Binding => "binding expression syntax is newer than this compiler",
            Self::Default => "Parameter default syntax is newer than this compiler",
            Self::Let => "let expression syntax is newer than this compiler",
        }
    }
}

pub(super) fn evaluate_parameter_expression(
    file: &str,
    expression: &Expr,
    context: ExpressionContext,
    resolve: &mut impl FnMut(&str, TextRange) -> Result<SymbolicParameterValue, Diagnostic>,
) -> Result<EvaluatedParameter, Diagnostic> {
    let evaluated = match expression.kind() {
        ExprKind::Number(value) => EvaluatedParameter {
            value: Some(normalize_zero(*value)),
            dimension: EvaluatedDimension::Known(DimExponents::DIMENSIONLESS),
            bare_literal: true,
            expression: Some(LoweringExpression::quantity(
                DynQuantity::new(normalize_zero(*value), DimExponents::DIMENSIONLESS),
                expression.range(),
            )),
            lineage: Some(ParameterLineage::Constant),
        },
        ExprKind::Quantity { value, unit } => {
            let quantity = crate::units::quantity(*value, unit).map_err(|message| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    message,
                )
            })?;
            EvaluatedParameter {
                value: Some(quantity.value()),
                dimension: EvaluatedDimension::Known(quantity.dim()),
                bare_literal: false,
                expression: Some(LoweringExpression::quantity(quantity, expression.range())),
                lineage: Some(ParameterLineage::Constant),
            }
        }
        ExprKind::Name(name) => resolve(name, expression.range())?.into(),
        ExprKind::Path(path) => match crate::math::constant(path) {
            Some(value) => EvaluatedParameter {
                value: Some(value),
                dimension: EvaluatedDimension::Known(DimExponents::DIMENSIONLESS),
                bare_literal: false,
                expression: Some(LoweringExpression::quantity(
                    DynQuantity::new(value, DimExponents::DIMENSIONLESS),
                    expression.range(),
                )),
                lineage: Some(ParameterLineage::Constant),
            },
            None => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    path.range(),
                    context.qualified_name_message(path),
                ));
            }
        },
        ExprKind::Call { callee, .. } => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                context.call_message(callee.as_str()),
            ));
        }
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => {
            let operand = evaluate_parameter_expression(file, value, context, resolve)?;
            let negated = operand
                .value
                .map(|value| finite_constant(file, expression.range(), -value))
                .transpose()?;
            EvaluatedParameter {
                value: negated,
                dimension: operand.dimension,
                bare_literal: operand.bare_literal,
                expression: operand
                    .expression
                    .map(|value| LoweringExpression::neg(value, expression.range())),
                lineage: transform_lineage(operand.lineage),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = evaluate_parameter_expression(file, left, context, resolve)?;
            let right = evaluate_parameter_expression(file, right, context, resolve)?;
            combine_parameters(file, expression.range(), *op, left, right)?
        }
        _ => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                expression.range(),
                context.unsupported_message(),
            ));
        }
    };
    Ok(evaluated)
}

pub(super) fn coerce_parameter(
    file: &str,
    range: TextRange,
    evaluated: EvaluatedParameter,
    target: DimExponents,
) -> Result<SymbolicParameterValue, Diagnostic> {
    coerce_parameter_with_label(file, range, evaluated, target, "Parameter binding")
}

pub(super) fn coerce_parameter_with_label(
    file: &str,
    range: TextRange,
    evaluated: EvaluatedParameter,
    target: DimExponents,
    label: &str,
) -> Result<SymbolicParameterValue, Diagnostic> {
    let dimension = if evaluated.bare_literal {
        EvaluatedDimension::Known(target)
    } else {
        evaluated.dimension
    };
    match dimension {
        EvaluatedDimension::Known(actual) if actual != target => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                format!("{label} has dimension [{}], expected [{}]", actual, target),
            ));
        }
        _ => {}
    }
    let expression = evaluated.expression.map(|expression| {
        if evaluated.bare_literal {
            expression.with_quantity_dimension(target)
        } else {
            expression
        }
    });
    Ok(SymbolicParameterValue {
        value: evaluated.value,
        dimension: target,
        expression,
        lineage: evaluated.lineage,
    })
}

pub(super) fn infer_parameter_with_label(
    file: &str,
    range: TextRange,
    evaluated: EvaluatedParameter,
    label: &str,
) -> Result<SymbolicParameterValue, Diagnostic> {
    let EvaluatedDimension::Known(dimension) = evaluated.dimension else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!(
                "{label} dimension cannot be inferred from this expression; add an explicit dimension annotation"
            ),
        ));
    };
    Ok(SymbolicParameterValue {
        value: evaluated.value,
        dimension,
        expression: evaluated.expression,
        lineage: evaluated.lineage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inferred_dimension_rejects_deferred_evaluation() {
        let error = infer_parameter_with_label(
            "deferred.eqi",
            TextRange::new(0, 1),
            EvaluatedParameter {
                value: None,
                dimension: EvaluatedDimension::Deferred,
                bare_literal: false,
                expression: None,
                lineage: None,
            },
            "let alias",
        )
        .expect_err("Deferred dimension requires an annotation");

        assert!(error.message().contains("dimension cannot be inferred"));
        assert!(error.message().contains("explicit dimension annotation"));
    }
}
