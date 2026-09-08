//! Resolve source decimal literals only after an exact operand context is known.
use super::*;
use eqiora_core::{ScalarDomain, ValueLiteral, ValueType};

pub(super) fn equation(
    file: &str,
    left: &LoweringExpression,
    right: &LoweringExpression,
    bindings: &BTreeMap<String, Binding>,
    support: Option<&SpatialSupport<RawId>>,
) -> Result<(LoweringExpression, LoweringExpression), Diagnostic> {
    let target =
        anchor(file, left, bindings, support).or_else(|| anchor(file, right, bindings, support));
    Ok((
        resolve(file, left, target, bindings, support)?,
        resolve(file, right, target, bindings, support)?,
    ))
}

fn anchor(
    file: &str,
    value: &LoweringExpression,
    bindings: &BTreeMap<String, Binding>,
    support: Option<&SpatialSupport<RawId>>,
) -> Option<ScalarDomain> {
    match value.node.as_ref() {
        LoweringExpressionNode::Number(_) => None,
        LoweringExpressionNode::Neg(value) => anchor(file, value, bindings, support),
        LoweringExpressionNode::Binary { left, right, .. } => {
            anchor(file, left, bindings, support).or_else(|| anchor(file, right, bindings, support))
        }
        _ => expression_type(file, value, bindings, support)
            .ok()
            .map(|ty| ty.value_type.scalar_domain()),
    }
}

fn resolve(
    file: &str,
    expression: &LoweringExpression,
    expected: Option<ScalarDomain>,
    bindings: &BTreeMap<String, Binding>,
    support: Option<&SpatialSupport<RawId>>,
) -> Result<LoweringExpression, Diagnostic> {
    let node = match expression.node.as_ref() {
        LoweringExpressionNode::Number(value) => return literal(file, expression, value, expected),
        LoweringExpressionNode::Neg(value) => {
            if let LoweringExpressionNode::Number(number) = value.node.as_ref() {
                let spelling = number.canonical_text();
                let signed = if let Some(value) = spelling.strip_prefix('-') {
                    value.to_owned()
                } else {
                    format!("-{spelling}")
                };
                let number = eqiora_lang::DecimalLiteral::parse(&signed).map_err(|error| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        expression.range(),
                        error.to_string(),
                    )
                })?;
                return literal(file, expression, &number, expected);
            }
            LoweringExpressionNode::Neg(resolve(file, value, expected, bindings, support)?)
        }
        LoweringExpressionNode::Binary {
            operator,
            left,
            right,
        } => {
            let domain = anchor(file, left, bindings, support)
                .or_else(|| anchor(file, right, bindings, support))
                .or(expected);
            LoweringExpressionNode::Binary {
                operator: *operator,
                left: resolve(file, left, domain, bindings, support)?,
                right: resolve(
                    file,
                    right,
                    if *operator == BinaryOp::Pow {
                        None
                    } else {
                        domain
                    },
                    bindings,
                    support,
                )?,
            }
        }
        LoweringExpressionNode::IntegerCall {
            operator,
            arguments,
        } => {
            if arguments.len() != operator.arity() {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    "invalid integer operation arity",
                ));
            }
            LoweringExpressionNode::IntegerCall {
                operator: *operator,
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        resolve(
                            file,
                            argument,
                            Some(operator.operand_domain()),
                            bindings,
                            support,
                        )
                    })
                    .collect::<Result<_, _>>()?,
            }
        }
        LoweringExpressionNode::Array(elements) => LoweringExpressionNode::Array(
            elements
                .iter()
                .map(|value| resolve(file, value, expected, bindings, support))
                .collect::<Result<_, _>>()?,
        ),
        LoweringExpressionNode::Index { value, index } => LoweringExpressionNode::Index {
            value: resolve(file, value, expected, bindings, support)?,
            index: *index,
        },
        LoweringExpressionNode::Complex { real, imag } => LoweringExpressionNode::Complex {
            real: resolve(file, real, Some(ScalarDomain::Real), bindings, support)?,
            imag: resolve(file, imag, Some(ScalarDomain::Real), bindings, support)?,
        },
        LoweringExpressionNode::Call { callee, argument } => LoweringExpressionNode::Call {
            callee: callee.clone(),
            argument: resolve(file, argument, None, bindings, support)?,
        },
        LoweringExpressionNode::Sample { value, clock } => LoweringExpressionNode::Sample {
            value: resolve(file, value, None, bindings, support)?,
            clock: clock.clone(),
        },
        LoweringExpressionNode::PureOperator {
            definition,
            arguments,
        } => LoweringExpressionNode::PureOperator {
            definition: definition.clone(),
            arguments: arguments
                .iter()
                .map(|value| resolve(file, value, None, bindings, support))
                .collect::<Result<_, _>>()?,
        },
        _ => return Ok(expression.clone()),
    };
    Ok(LoweringExpression {
        node: Arc::new(node),
        range: expression.range(),
    })
}
fn literal(
    file: &str,
    expression: &LoweringExpression,
    value: &eqiora_lang::DecimalLiteral,
    expected: Option<ScalarDomain>,
) -> Result<LoweringExpression, Diagnostic> {
    let literal = if expected == Some(ScalarDomain::Integer) {
        ValueLiteral::from_integer(
            ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS),
            value.to_i64().map_err(|error| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    error.to_string(),
                )
            })?,
        )
    } else {
        ValueLiteral::from_real(
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
            value.to_f64().map_err(|error| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    error.to_string(),
                )
            })?,
        )
    }
    .map_err(|error| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            expression.range(),
            error.to_string(),
        )
    })?;
    Ok(LoweringExpression::literal(literal, expression.range()))
}
