//! Lossless source expression projection before contextual value admission.
use super::*;

pub(in crate::lower) fn from_source(expression: &Expr) -> LoweringExpression {
    let kind = match expression.kind() {
        ExprKind::Array(elements) => LoweringExpressionNode::Array(
            elements
                .iter()
                .map(LoweringExpression::from_source)
                .collect(),
        ),
        ExprKind::Index { value, index } => match crate::hierarchy::closed_index(index) {
            Ok(index) => LoweringExpressionNode::Index {
                value: LoweringExpression::from_source(value),
                index,
            },
            Err(_) => LoweringExpressionNode::InvalidValue(
                "channel index requires a constant nonnegative integer",
            ),
        },
        ExprKind::Path(path) if path.as_str() == "math.i" => {
            return LoweringExpression::literal(
                eqiora_core::ValueLiteral::new(
                    eqiora_core::ValueType::scalar(
                        eqiora_core::ScalarDomain::Complex,
                        DimExponents::DIMENSIONLESS,
                    ),
                    [(0.0, 1.0)],
                )
                .expect("imaginary unit"),
                expression.range(),
            );
        }
        ExprKind::Call { callee, arguments } if callee.as_str() == "math.complex" => {
            match arguments.as_slice() {
                [real, imag] => LoweringExpressionNode::Complex {
                    real: LoweringExpression::from_source(real),
                    imag: LoweringExpression::from_source(imag),
                },
                _ => LoweringExpressionNode::InvalidValue(
                    "math.complex requires exactly two real scalar arguments",
                ),
            }
        }
        ExprKind::Quantity { value, unit } => match crate::units::quantity(value, unit) {
            Ok(value) => return LoweringExpression::quantity(value, expression.range()),
            Err(message) => LoweringExpressionNode::InvalidValue(message),
        },
        ExprKind::Boolean(value) => {
            LoweringExpressionNode::Literal(eqiora_core::ValueLiteral::boolean(*value))
        }
        ExprKind::Number(value) => LoweringExpressionNode::Number(value.clone()),
        ExprKind::Path(path) => match crate::math::constant(path) {
            Some(value) => {
                return LoweringExpression::quantity(
                    DynQuantity::new(value, DimExponents::DIMENSIONLESS),
                    expression.range(),
                );
            }
            None if crate::math::is_namespaced(path) => {
                LoweringExpressionNode::UnknownMath(path.as_str().to_owned())
            }
            None => LoweringExpressionNode::Unsupported,
        },
        ExprKind::Name(name) => LoweringExpressionNode::Name(name.clone()),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => return LoweringExpression::neg(from_source(value), expression.range()),
        ExprKind::Unary {
            op: UnaryOp::Not,
            value,
        } => LoweringExpressionNode::Not(from_source(value)),
        ExprKind::Binary { op, left, right } => LoweringExpressionNode::Binary {
            operator: *op,
            left: from_source(left),
            right: from_source(right),
        },
        ExprKind::Call { callee, arguments }
            if super::IntegerBuiltin::named(callee.as_str()).is_some() =>
        {
            let operator = super::IntegerBuiltin::named(callee.as_str()).unwrap();
            if arguments.len() != operator.arity() {
                LoweringExpressionNode::InvalidValue("invalid integer operation arity")
            } else {
                LoweringExpressionNode::IntegerCall {
                    operator,
                    arguments: arguments.iter().map(from_source).collect(),
                }
            }
        }
        ExprKind::Call { callee, arguments } if callee.as_str() == "sample" => {
            match arguments.as_slice() {
                [value, clock] => match clock.kind() {
                    ExprKind::Name(clock) => LoweringExpressionNode::Sample {
                        value: from_source(value),
                        clock: clock.clone(),
                    },
                    _ => LoweringExpressionNode::InvalidValue("sample requires one clock name"),
                },
                _ => LoweringExpressionNode::InvalidValue(
                    "sample requires a value and one clock name",
                ),
            }
        }
        ExprKind::Call { callee, arguments }
            if (!callee.is_qualified() || crate::math::is_namespaced(callee))
                && arguments.len() == 1 =>
        {
            LoweringExpressionNode::Call {
                callee: callee.as_str().to_owned(),
                argument: from_source(&arguments[0]),
            }
        }
        _ => LoweringExpressionNode::Unsupported,
    };
    LoweringExpression {
        node: Arc::new(kind),
        range: expression.range(),
    }
}
