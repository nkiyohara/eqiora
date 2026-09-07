//! Complete static values retain their authored expression dependencies.

use super::*;
use eqiora_schema::kernel::typing::{self, ExpressionType};

pub(super) fn evaluate(
    file: &str,
    expression: &Expr,
    context: ExpressionContext,
    resolve: &mut impl FnMut(&str, TextRange) -> Result<SymbolicParameterValue, Diagnostic>,
) -> Result<EvaluatedParameter, Diagnostic> {
    let error = |message: String| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            expression.range(),
            message,
        )
    };
    let evaluate = &mut |value: &Expr| evaluate_parameter_expression(file, value, context, resolve);
    let (operands, value_type, lowered, value) = match expression.kind() {
        ExprKind::Array(elements) => {
            let operands = elements
                .iter()
                .map(&mut *evaluate)
                .collect::<Result<Vec<_>, _>>()?;
            let types = operands
                .iter()
                .map(|value| ExpressionType::<()>::new(value.value_type.value_type().clone(), None))
                .collect::<Vec<_>>();
            let value_type = typing::array(&types)
                .map_err(|violation| error(violation.to_string()))?
                .value_type;
            crate::typed_values::check_type(&value_type).map_err(error)?;
            let lowered = operands
                .iter()
                .map(|value| value.expression.clone())
                .collect::<Option<Vec<_>>>()
                .map(|elements| LoweringExpression::array(elements, expression.range()));
            let value = operands
                .iter()
                .map(|value| value.value.as_ref())
                .collect::<Option<Vec<_>>>()
                .map(|values| {
                    ValueLiteral::new(
                        value_type.clone(),
                        values.iter().flat_map(|value| value.components()),
                    )
                    .map_err(|violation| error(violation.to_string()))
                })
                .transpose()?;
            (operands, value_type, lowered, value)
        }
        ExprKind::Index { value, index } => {
            let operand = evaluate(value)?;
            let index_value = evaluate(index)?;
            let index = checked_index(file, index.range(), &index_value)?;
            let value_type = typing::index(
                ExpressionType::<()>::new(operand.value_type.value_type().clone(), None),
                index,
            )
            .map_err(|violation| error(violation.to_string()))?
            .value_type;
            let lowered = operand
                .expression
                .clone()
                .map(|value| LoweringExpression::index(value, index, expression.range()));
            let value = operand
                .value
                .as_ref()
                .map(|value| {
                    let count = value_type
                        .shape()
                        .component_count()
                        .expect("checked element type");
                    ValueLiteral::new(
                        value_type.clone(),
                        value.components().skip(index as usize * count).take(count),
                    )
                    .map_err(|violation| error(violation.to_string()))
                })
                .transpose()?;
            (vec![operand], value_type, lowered, value)
        }
        ExprKind::Path(path) if path.as_str() == "math.i" => {
            let value_type = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS);
            let value =
                ValueLiteral::new(value_type.clone(), [(0.0, 1.0)]).expect("imaginary unit");
            (
                vec![],
                value_type,
                Some(LoweringExpression::literal(
                    value.clone(),
                    expression.range(),
                )),
                Some(value),
            )
        }
        ExprKind::Call { arguments, .. } => {
            let [real, imag] = arguments.as_slice() else {
                return Err(error(
                    "math.complex requires exactly two real scalar arguments".into(),
                ));
            };
            let real = evaluate(real)?;
            let imag = evaluate(imag)?;
            let value_type = typing::complex(
                ExpressionType::<()>::new(real.value_type.value_type().clone(), None),
                ExpressionType::new(imag.value_type.value_type().clone(), None),
            )
            .map_err(|violation| error(violation.to_string()))?
            .value_type;
            let lowered = real
                .expression
                .clone()
                .zip(imag.expression.clone())
                .map(|(real, imag)| LoweringExpression::complex(real, imag, expression.range()));
            let value = real
                .value
                .as_ref()
                .zip(imag.value.as_ref())
                .map(|(real, imag)| {
                    ValueLiteral::new(
                        value_type.clone(),
                        [(
                            real.component(0).expect("scalar").0,
                            imag.component(0).expect("scalar").0,
                        )],
                    )
                    .map_err(|violation| error(violation.to_string()))
                })
                .transpose()?;
            (vec![real, imag], value_type, lowered, value)
        }
        _ => unreachable!("only complete value constructors reach this owner"),
    };
    let known = operands
        .iter()
        .all(|operand| matches!(operand.value_type, EvaluatedType::Known(_)));
    let lineage = operands
        .iter()
        .fold(Some(ParameterLineage::Constant), |lineage, value| {
            combine_lineages(lineage, value.lineage.clone())
        });
    Ok(EvaluatedParameter {
        value,
        value_type: if known {
            EvaluatedType::Known(value_type)
        } else {
            EvaluatedType::Deferred(value_type)
        },
        bare_literal: false,
        expression: lowered,
        lineage,
    })
}

pub(super) fn checked_index(
    file: &str,
    range: TextRange,
    value: &EvaluatedParameter,
) -> Result<u32, Diagnostic> {
    let invalid = || {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "channel index requires a constant dimensionless nonnegative integer; live Parameters cannot select an index",
        )
    };
    if !matches!(value.lineage, Some(ParameterLineage::Constant)) {
        return Err(invalid());
    }
    let quantity = value
        .value
        .as_ref()
        .and_then(ValueLiteral::real_scalar_value)
        .ok_or_else(invalid)?;
    if quantity.dim() != DimExponents::DIMENSIONLESS
        || quantity.value() < 0.0
        || quantity.value() > f64::from(u32::MAX)
        || quantity.value().fract() != 0.0
    {
        return Err(invalid());
    }
    Ok(quantity.value() as u32)
}
